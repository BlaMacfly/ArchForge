//! Connexion à MariaDB et exécution des requêtes de l'éditeur.
//!
//! Les bases d'ArchCore sont désignées par un alias (`auth`, `characters`, `world`, `logs`) : l'interface
//! n'écrit jamais le nom réel d'une base dans ses requêtes, si bien que le SQL qu'elle produit se rejoue
//! tel quel ailleurs (couche de contenu, autre serveur).

use anyhow::{anyhow, bail, Context, Result};
use mysql_async::consts::ColumnType;
use mysql_async::prelude::*;
use mysql_async::{Column, Conn, OptsBuilder, Pool, PoolConstraints, PoolOpts, Row, Value};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};

/// Lignes renvoyées au plus par un jeu de résultats (l'éditeur pagine ses recherches).
const MAX_ROWS: usize = 5000;
/// Au-delà, un entier perd sa précision dans un nombre JavaScript : il part en texte.
const JS_SAFE_INT: i128 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
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

pub struct Db {
    pool: Pool,
    pub settings: Settings,
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct ResultSet {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Json>>,
    pub affected: u64,
    pub truncated: bool,
}

impl Db {
    pub async fn connect(settings: Settings) -> Result<Db> {
        let constraints = PoolConstraints::new(0, 4).expect("bornes du pool");
        let opts = OptsBuilder::default()
            .ip_or_hostname(settings.host.clone())
            .tcp_port(settings.port)
            .user(Some(settings.user.clone()))
            .pass(Some(settings.password.clone()))
            .pool_opts(PoolOpts::default().with_constraints(constraints));
        let pool = Pool::new(opts);
        let mut conn = pool.get_conn().await.context("connexion à MariaDB impossible")?;
        let version: Option<String> = conn.query_first("SELECT VERSION()").await?;
        drop(conn);
        Ok(Db { pool, settings, version: version.unwrap_or_default() })
    }

    pub async fn disconnect(self) {
        let _ = self.pool.disconnect().await;
    }

    async fn conn(&self, alias: &str) -> Result<Conn> {
        let schema = self.settings.schema(alias)?;
        let mut conn = self.pool.get_conn().await?;
        conn.query_drop(format!("USE `{}`", schema.replace('`', "``"))).await?;
        Ok(conn)
    }

    /// Exécute une requête de lecture paramétrée (une seule instruction, `?` positionnels).
    pub async fn select(&self, alias: &str, sql: &str, params: &[Json]) -> Result<ResultSet> {
        let mut conn = self.conn(alias).await?;
        let params: Vec<Value> = params.iter().map(json_to_param).collect::<Result<_>>()?;
        let mut result = conn.exec_iter(sql, params).await?;
        let columns = result.columns().unwrap_or_else(|| Vec::<Column>::new().into());
        let rows: Vec<Row> = result.collect().await?;
        Ok(to_result_set(&columns, rows, 0))
    }

    /// Exécute un script (plusieurs instructions possibles). En écriture, le script passe dans une
    /// transaction : tout ou rien, comme `archcore content` pour la couche de contenu.
    pub async fn script(&self, alias: &str, sql: &str, transaction: bool) -> Result<Vec<ResultSet>> {
        let mut conn = self.conn(alias).await?;
        if transaction {
            conn.query_drop("START TRANSACTION").await?;
        }
        match run_script(&mut conn, sql).await {
            Ok(sets) => {
                if transaction {
                    conn.query_drop("COMMIT").await?;
                }
                Ok(sets)
            }
            Err(err) => {
                if transaction {
                    let _ = conn.query_drop("ROLLBACK").await;
                }
                Err(err)
            }
        }
    }

    /// État des bases : nombre de lignes par table et versions du schéma.
    pub async fn overview(&self) -> Result<Json> {
        let mut conn = self.pool.get_conn().await?;
        let mut bases = serde_json::Map::new();
        for alias in ["auth", "characters", "world", "logs"] {
            let Ok(schema) = self.settings.schema(alias) else { continue };
            let tables: Vec<(String, Option<u64>)> = conn
                .exec(
                    "SELECT table_name, table_rows FROM information_schema.tables \
                     WHERE table_schema = ? ORDER BY table_name",
                    (schema,),
                )
                .await?;
            let migrations: Vec<(String, String)> = if tables.iter().any(|(t, _)| t == "schema_migrations") {
                conn.query(format!(
                    "SELECT id, CAST(applied_at AS CHAR) FROM `{}`.schema_migrations ORDER BY id",
                    schema.replace('`', "``")
                ))
                .await?
            } else {
                Vec::new()
            };
            bases.insert(
                alias.to_string(),
                json!({
                    "schema": schema,
                    "exists": !tables.is_empty(),
                    "tables": tables.iter().map(|(t, n)| json!({ "name": t, "rows": n })).collect::<Vec<_>>(),
                    "migrations": migrations.iter().map(|(id, at)| json!({ "id": id, "applied_at": at })).collect::<Vec<_>>(),
                }),
            );
        }
        Ok(Json::Object(bases))
    }
}

async fn run_script(conn: &mut Conn, sql: &str) -> Result<Vec<ResultSet>> {
    let mut result = conn.query_iter(sql).await?;
    let mut sets = Vec::new();
    while !result.is_empty() {
        let columns = result.columns().unwrap_or_else(|| Vec::<Column>::new().into());
        let rows: Vec<Row> = result.collect().await?;
        let affected = result.affected_rows();
        sets.push(to_result_set(&columns, rows, affected));
    }
    Ok(sets)
}

fn to_result_set(columns: &[Column], rows: Vec<Row>, affected: u64) -> ResultSet {
    let truncated = rows.len() > MAX_ROWS;
    let rows = rows
        .into_iter()
        .take(MAX_ROWS)
        .map(|row| {
            let values = row.unwrap();
            values.into_iter().zip(columns).map(|(v, c)| value_to_json(v, c)).collect()
        })
        .collect();
    ResultSet {
        columns: columns.iter().map(|c| c.name_str().into_owned()).collect(),
        rows,
        affected,
        truncated,
    }
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

fn int_to_json(n: i128) -> Json {
    if n.abs() > JS_SAFE_INT {
        Json::String(n.to_string())
    } else {
        json!(n as i64)
    }
}

fn value_to_json(value: Value, column: &Column) -> Json {
    match value {
        Value::NULL => Json::Null,
        Value::Int(n) => int_to_json(n as i128),
        Value::UInt(n) => int_to_json(n as i128),
        Value::Float(f) => json!(f),
        Value::Double(f) => json!(f),
        Value::Bytes(bytes) => {
            if is_binary(column) {
                return json!({ "binary": bytes.len() });
            }
            let text = String::from_utf8_lossy(&bytes).into_owned();
            let t = column.column_type();
            if is_integer(t) {
                if let Ok(n) = text.parse::<i128>() {
                    return int_to_json(n);
                }
            } else if is_real(t) {
                if let Ok(f) = text.parse::<f64>() {
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
    fn les_grands_entiers_partent_en_texte() {
        assert_eq!(int_to_json(42), json!(42));
        assert_eq!(int_to_json(18_446_744_073_709_551_615), json!("18446744073709551615"));
    }
}
