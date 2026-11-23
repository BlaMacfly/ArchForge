//! Connexion à MariaDB et exécution des requêtes de l'éditeur.
//!
//! Les bases d'ArchCore sont désignées par un alias (`auth`, `characters`, `world`, `logs`) : l'éditeur
//! n'écrit jamais le nom réel d'une base dans ses requêtes, si bien que le SQL qu'il produit se rejoue tel
//! quel ailleurs (couche de contenu, autre serveur).
//!
//! À la connexion, ArchForge lit le schéma réel (`information_schema`) : colonnes, types, clés et
//! commentaires. Une colonne ajoutée par une migration d'ArchCore apparaît donc dans l'éditeur, expliquée par
//! son commentaire, même si ArchForge ne la connaît pas encore.

use anyhow::{anyhow, bail, Context, Result};
use mysql::consts::ColumnType;
use mysql::prelude::*;
use mysql::{Column, OptsBuilder, Pool, PoolConstraints, PoolOpts, PooledConn, Value};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};
use std::collections::HashMap;

use crate::sql::Row;

/// Lignes renvoyées au plus par un jeu de résultats (l'éditeur pagine ses recherches).
const MAX_ROWS: usize = 5000;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Settings {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub auth: String,
    pub characters: String,
    pub world: String,
    pub logs: String,
}

/// Bases d'ArchCore, dans l'ordre d'affichage.
pub const ALIASES: [(&str, &str); 4] = [("world", "Monde"), ("auth", "Comptes"), ("characters", "Personnages"), ("logs", "Journaux")];

impl Settings {
    /// Nom réel de la base désignée par un alias.
    pub fn schema(&self, alias: &str) -> Result<&str> {
        let name = match alias {
            "auth" => &self.auth,
            "characters" => &self.characters,
            "world" => &self.world,
            "logs" => &self.logs,
            other => bail!("base inconnue : {other}"),
        };
        if name.is_empty() {
            bail!("la base « {alias} » n'est pas configurée");
        }
        Ok(name)
    }
}

/// Une colonne du schéma réel.
#[derive(Clone, Debug)]
pub struct ColumnInfo {
    pub name: String,
    /// Type SQL sans taille (`int`, `varchar`, `longtext`…).
    pub data_type: String,
    pub nullable: bool,
    pub comment: String,
}

impl ColumnInfo {
    pub fn is_integer(&self) -> bool {
        matches!(self.data_type.as_str(), "tinyint" | "smallint" | "mediumint" | "int" | "bigint" | "year")
    }
    pub fn is_real(&self) -> bool {
        matches!(self.data_type.as_str(), "float" | "double" | "decimal")
    }
    pub fn is_binary(&self) -> bool {
        self.data_type.contains("blob") || self.data_type.contains("binary")
    }
}

/// Schéma réel des bases : alias → table → colonnes (dans l'ordre de la table).
#[derive(Clone, Debug, Default)]
pub struct Schema {
    pub tables: HashMap<String, HashMap<String, Vec<ColumnInfo>>>,
}

impl Schema {
    pub fn columns(&self, alias: &str, table: &str) -> Option<&[ColumnInfo]> {
        self.tables.get(alias)?.get(table).map(|v| v.as_slice())
    }
    pub fn has_table(&self, alias: &str, table: &str) -> bool {
        self.columns(alias, table).is_some()
    }
}

#[derive(Clone, Debug, Default)]
pub struct ResultSet {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Json>>,
    pub affected: u64,
    pub truncated: bool,
}

impl ResultSet {
    /// Lignes sous forme de `{ colonne: valeur }`.
    pub fn maps(&self) -> Vec<Row> {
        self.rows.iter().map(|r| self.columns.iter().cloned().zip(r.iter().cloned()).collect()).collect()
    }
}

/// État d'une base pour le tableau de bord.
#[derive(Clone, Debug, Default)]
pub struct BaseInfo {
    pub alias: String,
    pub schema: String,
    pub exists: bool,
    pub tables: Vec<(String, u64)>,
    pub last_migration: Option<String>,
    pub migrations: usize,
}

pub struct Db {
    pool: Pool,
    pub settings: Settings,
    pub version: String,
    pub schema: Schema,
}

impl Db {
    pub fn connect(settings: Settings) -> Result<Db> {
        let opts = OptsBuilder::new()
            .ip_or_hostname(Some(settings.host.clone()))
            .tcp_port(settings.port)
            .user(Some(settings.user.clone()))
            .pass(Some(settings.password.clone()))
            .tcp_connect_timeout(Some(std::time::Duration::from_secs(5)))
            .pool_opts(PoolOpts::default().with_constraints(PoolConstraints::new(0, 4).expect("bornes du pool")));
        let pool = Pool::new(opts).context("connexion à MariaDB impossible")?;
        let mut conn = pool.get_conn().context("connexion à MariaDB impossible")?;
        let version: Option<String> = conn.query_first("SELECT VERSION()")?;
        let mut db = Db { pool, settings, version: version.unwrap_or_default(), schema: Schema::default() };
        db.schema = db.load_schema(&mut conn)?;
        if !db.schema.has_table("world", "item_data") {
            bail!(
                "la base monde « {} » ne contient pas les tables d'ArchCore : lancer `archcore db-init` puis `archcore import`",
                db.settings.world
            );
        }
        Ok(db)
    }

    fn load_schema(&self, conn: &mut PooledConn) -> Result<Schema> {
        let mut schema = Schema::default();
        for (alias, _) in ALIASES {
            let Ok(name) = self.settings.schema(alias) else { continue };
            let cols: Vec<(String, String, String, String, String)> = conn.exec(
                "SELECT table_name, column_name, data_type, is_nullable, column_comment \
                 FROM information_schema.columns WHERE table_schema = ? ORDER BY table_name, ordinal_position",
                (name,),
            )?;
            let tables = schema.tables.entry(alias.to_string()).or_default();
            for (table, column, data_type, nullable, comment) in cols {
                tables.entry(table).or_default().push(ColumnInfo {
                    name: column,
                    data_type: data_type.to_lowercase(),
                    nullable: nullable == "YES",
                    comment,
                });
            }
        }
        Ok(schema)
    }

    fn conn(&self, alias: &str) -> Result<PooledConn> {
        let schema = self.settings.schema(alias)?;
        let mut conn = self.pool.get_conn()?;
        conn.query_drop(format!("USE `{}`", schema.replace('`', "``")))?;
        Ok(conn)
    }

    /// Requête de lecture paramétrée (une seule instruction, `?` positionnels).
    pub fn select(&self, alias: &str, sql: &str, params: &[Json]) -> Result<ResultSet> {
        let mut conn = self.conn(alias)?;
        let params: Vec<Value> = params.iter().map(json_to_param).collect::<Result<_>>()?;
        let params = if params.is_empty() { mysql::Params::Empty } else { mysql::Params::Positional(params) };
        let mut result = conn.exec_iter(sql, params)?;
        let Some(set) = result.iter() else { return Ok(ResultSet::default()) };
        let columns: Vec<Column> = set.columns().as_ref().to_vec();
        let rows = set.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(to_result_set(&columns, rows, 0))
    }

    pub fn select_maps(&self, alias: &str, sql: &str, params: &[Json]) -> Result<Vec<Row>> {
        Ok(self.select(alias, sql, params)?.maps())
    }

    /// Script (plusieurs instructions possibles). En écriture, il passe dans une transaction : tout ou rien,
    /// comme `archcore content` pour la couche de contenu.
    pub fn script(&self, alias: &str, sql: &str, transaction: bool) -> Result<Vec<ResultSet>> {
        let mut conn = self.conn(alias)?;
        if transaction {
            conn.query_drop("START TRANSACTION")?;
        }
        match run_script(&mut conn, sql) {
            Ok(sets) => {
                if transaction {
                    conn.query_drop("COMMIT")?;
                }
                Ok(sets)
            }
            Err(err) => {
                if transaction {
                    let _ = conn.query_drop("ROLLBACK");
                }
                Err(err)
            }
        }
    }

    /// État des bases : nombre de lignes par table et versions du schéma.
    pub fn overview(&self) -> Result<Vec<BaseInfo>> {
        let mut conn = self.pool.get_conn()?;
        let mut out = Vec::new();
        for (alias, _) in ALIASES {
            let Ok(schema) = self.settings.schema(alias) else { continue };
            let tables: Vec<(String, Option<u64>)> = conn.exec(
                "SELECT table_name, table_rows FROM information_schema.tables WHERE table_schema = ? ORDER BY table_name",
                (schema,),
            )?;
            let (mut last_migration, mut migrations) = (None, 0);
            if tables.iter().any(|(t, _)| t == "schema_migrations") {
                let ids: Vec<String> =
                    conn.query(format!("SELECT id FROM `{}`.schema_migrations ORDER BY id", schema.replace('`', "``")))?;
                migrations = ids.len();
                last_migration = ids.last().cloned();
            }
            out.push(BaseInfo {
                alias: alias.to_string(),
                schema: schema.to_string(),
                exists: !tables.is_empty(),
                tables: tables.into_iter().map(|(t, n)| (t, n.unwrap_or(0))).collect(),
                last_migration,
                migrations,
            });
        }
        Ok(out)
    }
}

fn run_script(conn: &mut PooledConn, sql: &str) -> Result<Vec<ResultSet>> {
    let mut result = conn.query_iter(sql)?;
    let mut sets = Vec::new();
    while let Some(set) = result.iter() {
        let columns: Vec<Column> = set.columns().as_ref().to_vec();
        let affected = set.affected_rows();
        let rows = set.collect::<std::result::Result<Vec<_>, _>>()?;
        sets.push(to_result_set(&columns, rows, affected));
    }
    Ok(sets)
}

fn to_result_set(columns: &[Column], rows: Vec<mysql::Row>, affected: u64) -> ResultSet {
    let truncated = rows.len() > MAX_ROWS;
    let rows = rows
        .into_iter()
        .take(MAX_ROWS)
        .map(|row| row.unwrap().into_iter().zip(columns).map(|(v, c)| value_to_json(v, c)).collect())
        .collect();
    ResultSet { columns: columns.iter().map(|c| c.name_str().into_owned()).collect(), rows, affected, truncated }
}

fn is_integer(t: ColumnType) -> bool {
    use ColumnType::*;
    matches!(t, MYSQL_TYPE_TINY | MYSQL_TYPE_SHORT | MYSQL_TYPE_LONG | MYSQL_TYPE_LONGLONG | MYSQL_TYPE_INT24 | MYSQL_TYPE_YEAR)
}

fn is_real(t: ColumnType) -> bool {
    use ColumnType::*;
    matches!(t, MYSQL_TYPE_FLOAT | MYSQL_TYPE_DOUBLE | MYSQL_TYPE_DECIMAL | MYSQL_TYPE_NEWDECIMAL)
}

fn is_binary(c: &Column) -> bool {
    use ColumnType::*;
    // Jeu de caractères 63 : « binary ». Les BLOB de la carte (map_height…) ne s'affichent pas.
    c.character_set() == 63
        && matches!(
            c.column_type(),
            MYSQL_TYPE_BLOB | MYSQL_TYPE_TINY_BLOB | MYSQL_TYPE_MEDIUM_BLOB | MYSQL_TYPE_LONG_BLOB | MYSQL_TYPE_VAR_STRING | MYSQL_TYPE_STRING
        )
}

/// Marque d'une valeur binaire (non modifiable) dans une ligne.
pub fn binary_len(v: &Json) -> Option<u64> {
    v.as_object().filter(|o| o.len() == 1).and_then(|o| o.get("binary")).and_then(|n| n.as_u64())
}

fn value_to_json(value: Value, column: &Column) -> Json {
    match value {
        Value::NULL => Json::Null,
        Value::Int(n) => json!(n),
        Value::UInt(n) => json!(n),
        Value::Float(f) => json!(f),
        Value::Double(f) => json!(f),
        Value::Bytes(bytes) => {
            if is_binary(column) {
                return json!({ "binary": bytes.len() });
            }
            let text = String::from_utf8_lossy(&bytes).into_owned();
            let t = column.column_type();
            if is_integer(t) {
                if let Ok(n) = text.parse::<i64>() {
                    return json!(n);
                }
                if let Ok(n) = text.parse::<u64>() {
                    return json!(n);
                }
            } else if is_real(t) {
                if let Ok(f) = text.parse::<f64>() {
                    if f.fract() == 0.0 && f.abs() < 1e15 {
                        return json!(f as i64);
                    }
                    return json!(f);
                }
            }
            Json::String(text)
        }
        Value::Date(y, mo, d, h, mi, s, _) => {
            if column.column_type() == ColumnType::MYSQL_TYPE_DATE {
                Json::String(format!("{y:04}-{mo:02}-{d:02}"))
            } else {
                Json::String(format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}"))
            }
        }
        Value::Time(neg, days, h, mi, s, _) => {
            let h = days * 24 + h as u32;
            Json::String(format!("{}{h:02}:{mi:02}:{s:02}", if neg { "-" } else { "" }))
        }
    }
}

fn json_to_param(value: &Json) -> Result<Value> {
    Ok(match value {
        Json::Null => Value::NULL,
        Json::Bool(b) => Value::Int(*b as i64),
        Json::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(u) = n.as_u64() {
                Value::UInt(u)
            } else {
                Value::Double(n.as_f64().ok_or_else(|| anyhow!("nombre invalide"))?)
            }
        }
        Json::String(s) => Value::Bytes(s.as_bytes().to_vec()),
        other => Value::Bytes(other.to_string().into_bytes()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_alias_designent_les_bases_configurees() {
        let s = Settings { world: "archcore_world".into(), ..Default::default() };
        assert_eq!(s.schema("world").unwrap(), "archcore_world");
        assert!(s.schema("auth").is_err(), "base vide : refusée");
        assert!(s.schema("mysql").is_err(), "alias inconnu : refusé");
    }

    #[test]
    fn marque_des_valeurs_binaires() {
        assert_eq!(binary_len(&json!({"binary": 578})), Some(578));
        assert_eq!(binary_len(&json!({"binary": 578, "x": 1})), None);
        assert_eq!(binary_len(&json!("binary")), None);
    }
}
