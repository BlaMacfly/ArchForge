//! Moteur des éditeurs : chargement d'une entrée (sur un fil à part), état de chaque onglet, contrôles et
//! requêtes produites — l'équivalent des services `SingleRowEditorService` et `MultiRowEditorService` de
//! Keira3. L'affichage est dans `ui/editor.rs`.

use anyhow::Result;
use serde_json::Value;

use crate::db::Db;
use crate::model::{Entity, Field, Id, Kind, Tab, TabKind};
use crate::sql::{self, Cond, Key, Opts, Row, RowChange};

#[derive(Clone, Debug)]
pub struct RowState {
    pub before: Option<Row>,
    pub after: Row,
    pub deleted: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RowStatus {
    Same,
    Added,
    Changed,
    Deleted,
}

impl RowState {
    pub fn status(&self) -> RowStatus {
        match (&self.before, self.deleted) {
            (_, true) => RowStatus::Deleted,
            (None, _) => RowStatus::Added,
            (Some(b), _) if rows_equal(b, &self.after) => RowStatus::Same,
            _ => RowStatus::Changed,
        }
    }
}

fn rows_equal(a: &Row, b: &Row) -> bool {
    sql::deep_equal(&Value::Object(a.clone()), &Value::Object(b.clone()))
}

#[derive(Clone, Debug)]
pub enum TabData {
    Single { before: Option<Row>, after: Option<Row> },
    Multi { rows: Vec<RowState>, columns: Vec<String> },
    List { rows: Vec<Row> },
}

/// Colonnes JSON lues en texte par MariaDB : remises en objets pour l'éditeur.
fn parse_json(mut row: Row, cols: &[&str]) -> Row {
    for c in cols {
        if let Some(Value::String(s)) = row.get(*c) {
            if let Ok(v) = serde_json::from_str::<Value>(s) {
                row.insert(c.to_string(), v);
            }
        }
    }
    row
}

fn where_params(key: &Key) -> (String, Vec<Value>) {
    let mut params = Vec::new();
    let sql = key
        .iter()
        .map(|(c, cond)| match cond {
            Cond::Eq(v) => {
                params.push(v.clone());
                format!("{} = ?", sql::ident(c))
            }
            Cond::In(vs) => {
                params.extend(vs.iter().cloned());
                format!("{} IN ({})", sql::ident(c), vec!["?"; vs.len()].join(", "))
            }
        })
        .collect::<Vec<_>>()
        .join(" AND ");
    (sql, params)
}

/// Colonnes de rattachement à recopier dans une ligne (une liste de valeurs n'en fixe aucune).
pub fn attach(key: &Key) -> Row {
    key.iter()
        .filter_map(|(c, cond)| match cond {
            Cond::Eq(v) => Some((c.clone(), v.clone())),
            Cond::In(_) => None,
        })
        .collect()
}

fn fetch_rows(db: &Db, alias: &str, table: &str, key: &Key, order: &str, json: &[&str]) -> Result<(Vec<Row>, Vec<String>)> {
    let (w, params) = where_params(key);
    let order = if order.is_empty() { String::new() } else { format!(" ORDER BY {order}") };
    let set = db.select(alias, &format!("SELECT * FROM {} WHERE {w}{order}", sql::ident(table)), &params)?;
    let columns = set.columns.clone();
    Ok((set.maps().into_iter().map(|r| parse_json(r, json)).collect(), columns))
}

/// Charge tous les onglets d'une entrée. `copy_from` : entrée dont une création reprend les valeurs.
pub fn load(db: &Db, entity: &Entity, id: &Id, is_new: bool, copy_from: Option<&Id>) -> Result<Vec<TabData>> {
    let mut out = Vec::new();
    for tab in &entity.tabs {
        let alias = entity.tab_db(tab);
        let json = tab.json_columns();
        // Table absente (base d'une version plus ancienne d'ArchCore) : onglet vide.
        if let Some(t) = tab.table() {
            if !db.schema.has_table(alias, t) {
                out.push(TabData::List { rows: Vec::new() });
                continue;
            }
        }
        out.push(match &tab.kind {
            TabKind::Single(s) => {
                let before = if is_new { None } else { fetch_rows(db, alias, s.table, &(s.key)(id), "", &json)?.0.into_iter().next() };
                let mut after = before.clone();
                if is_new {
                    let source = match copy_from {
                        Some(c) => fetch_rows(db, alias, s.table, &(s.key)(c), "", &json)?.0.into_iter().next(),
                        None => None,
                    };
                    if let Some(mut src) = source {
                        src.extend(attach(&(s.key)(id)));
                        after = Some(match s.on_copy {
                            Some(adjust) => adjust(src, id),
                            None => src,
                        });
                    } else if s.main {
                        let mut row = attach(&(s.key)(id));
                        row.extend((s.defaults)(id));
                        after = Some(row);
                    }
                }
                TabData::Single { before, after }
            }
            TabKind::Multi(m) => {
                let (loaded, columns) = fetch_rows(db, alias, m.table, &(m.parent)(id), m.order, &json)?;
                let mut rows: Vec<RowState> = if is_new {
                    Vec::new()
                } else {
                    loaded.into_iter().map(|r| RowState { before: Some(r.clone()), after: r, deleted: false }).collect()
                };
                if is_new && m.copy {
                    if let Some(c) = copy_from {
                        let (source, _) = fetch_rows(db, alias, m.table, &(m.parent)(c), m.order, &json)?;
                        rows = source.into_iter().map(|r| RowState { before: None, after: new_row_from(m, id, r), deleted: false }).collect();
                    }
                }
                TabData::Multi { rows, columns }
            }
            TabKind::List(l) => {
                if is_new {
                    TabData::List { rows: Vec::new() }
                } else {
                    let (sql, params) = (l.query)(id);
                    TabData::List { rows: db.select_maps(alias, &sql, &params)? }
                }
            }
        });
    }
    Ok(out)
}

/// Ligne neuve tirée d'une autre (copie, duplication) : rattachée à l'entrée, clés régénérées.
pub fn new_row_from(m: &crate::model::MultiTab, id: &Id, mut row: Row) -> Row {
    row.extend(attach(&(m.parent)(id)));
    if let Some(col) = m.auto_id {
        row.insert(col.to_string(), Value::Null);
    }
    if let Some(col) = m.auto_increment {
        row.remove(col);
    }
    match m.fresh {
        Some(fresh) => fresh(row),
        None => row,
    }
}

/// Ligne neuve ajoutée par « Ajouter une ligne ».
pub fn add_row(m: &crate::model::MultiTab, id: &Id, rows: &[RowState]) -> Row {
    let current: Vec<Row> = rows.iter().filter(|r| !r.deleted).map(|r| r.after.clone()).collect();
    let mut row = (m.new_row)(id, &current, current.last());
    row.extend(attach(&(m.parent)(id)));
    if let Some(col) = m.auto_id {
        row.insert(col.to_string(), Value::Null);
    }
    if let Some(col) = m.auto_increment {
        row.remove(col);
    }
    match m.fresh {
        Some(fresh) => fresh(row),
        None => row,
    }
}

// ------------------------------------------------------------------ contrôles

/// Valeur saisie → valeur typée : un nombre valide devient un nombre, le reste reste du texte (signalé).
pub fn parse_input(text: &str, field: &Field) -> Value {
    let t = text.trim();
    if t.is_empty() {
        return if field.nullable { Value::Null } else { Value::String(String::new()) };
    }
    let numeric = field.kind.is_numeric() || matches!(field.kind, Kind::Float);
    if numeric {
        if let Ok(n) = t.parse::<i64>() {
            return Value::from(n);
        }
        if let Ok(n) = t.parse::<u64>() {
            return Value::from(n);
        }
        if matches!(field.kind, Kind::Float) {
            if let Ok(x) = t.parse::<f64>() {
                if x.is_finite() && format_number(x) == t {
                    return Value::from(x);
                }
            }
        }
        return Value::String(text.to_string());
    }
    Value::String(text.to_string())
}

pub fn format_number(x: f64) -> String {
    serde_json::Number::from_f64(x).map(|n| n.to_string()).unwrap_or_default()
}

/// Texte affiché pour une valeur simple.
pub fn display(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

pub fn is_valid(v: &Value, field: &Field) -> bool {
    if field.kind.is_json() || field.readonly {
        return true;
    }
    match v {
        Value::Null => field.nullable,
        Value::String(s) if s.is_empty() && field.allow_empty => true,
        Value::Number(_) => true,
        Value::String(s) => {
            if field.kind.is_numeric() {
                s.trim().parse::<i64>().is_ok() || s.trim().parse::<u64>().is_ok()
            } else if matches!(field.kind, Kind::Float) {
                s.trim().parse::<f64>().is_ok()
            } else {
                true
            }
        }
        _ => true,
    }
}

/// Champs à contrôler d'un onglet : déclarés, et présents dans la table.
fn checked_fields<'a>(fields: &'a [Field], columns: &[String]) -> impl Iterator<Item = &'a Field> {
    let cols: Vec<String> = columns.to_vec();
    fields.iter().filter(move |f| cols.is_empty() || cols.iter().any(|c| c == f.name))
}

pub fn errors(tab: &Tab, data: &TabData) -> Vec<String> {
    let mut errs = Vec::new();
    match (&tab.kind, data) {
        (TabKind::Single(s), TabData::Single { after: Some(after), .. }) => {
            let cols: Vec<String> = after.keys().cloned().collect();
            for f in checked_fields(&s.fields, &cols) {
                if !is_valid(after.get(f.name).unwrap_or(&Value::Null), f) {
                    errs.push(f.label.to_string());
                }
            }
        }
        (TabKind::Multi(m), TabData::Multi { rows, columns }) => {
            for (i, r) in rows.iter().enumerate() {
                if r.deleted {
                    continue;
                }
                for f in checked_fields(&m.columns, columns) {
                    if !is_valid(r.after.get(f.name).unwrap_or(&Value::Null), f) {
                        errs.push(format!("ligne {} : {}", i + 1, f.label));
                    }
                }
            }
            let missing = |row: &Row| {
                m.row_key.iter().any(|k| matches!(row.get(*k), None | Some(Value::Null)) || row.get(*k) == Some(&Value::String(String::new())))
            };
            // Une ligne sans clé ne peut être visée : UPDATE ou DELETE toucheraient d'autres lignes.
            for (i, r) in rows.iter().enumerate() {
                if let Some(b) = &r.before {
                    if missing(b) && r.status() != RowStatus::Same {
                        errs.push(format!("ligne {} : clé {} vide en base (relancer `archcore import`)", i + 1, m.row_key.join(", ")));
                    }
                }
            }
            let keys: Vec<String> = rows
                .iter()
                .filter(|r| !r.deleted && !missing(&r.after) && !(r.before.is_none() && m.auto_increment.is_some()))
                .map(|r| m.row_key.iter().map(|k| r.after.get(*k).map(sql::lit).unwrap_or_default()).collect::<Vec<_>>().join("\u{1}"))
                .collect();
            let mut unique = keys.clone();
            unique.sort();
            unique.dedup();
            if unique.len() != keys.len() {
                errs.push(format!("deux lignes ont la même clé ({})", m.row_key.join(", ")));
            }
        }
        _ => {}
    }
    errs
}

// ------------------------------------------------------------------ requêtes

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// Requête des changements de l'onglet (vide : rien à écrire).
pub fn diff_sql(tab: &Tab, data: &TabData, id: &Id) -> String {
    let json = tab.json_columns();
    let opts = Opts { columns: None, json: &json };
    match (&tab.kind, data) {
        (TabKind::Single(s), TabData::Single { before, after: Some(after) }) => {
            let key_cols: Vec<String> = attach(&(s.key)(id)).keys().cloned().collect();
            sql::single_row_diff(s.table, &key_cols, before.as_ref(), after, &opts)
        }
        (TabKind::Multi(m), TabData::Multi { rows, .. }) => {
            let changes: Vec<RowChange> = rows.iter().map(|r| RowChange { before: r.before.as_ref(), after: &r.after, deleted: r.deleted }).collect();
            sql::multi_row_diff(m.table, &strings(&m.row_key), &changes, &opts, m.auto_id)
        }
        _ => String::new(),
    }
}

/// Requête de l'entrée complète de l'onglet (DELETE puis INSERT).
pub fn full_sql(tab: &Tab, data: &TabData, id: &Id) -> String {
    let json = tab.json_columns();
    let opts = Opts { columns: None, json: &json };
    match (&tab.kind, data) {
        (TabKind::Single(s), TabData::Single { after: Some(after), .. }) => {
            let key_cols: Vec<String> = attach(&(s.key)(id)).keys().cloned().collect();
            sql::single_row_full(s.table, &key_cols, after, &opts)
        }
        (TabKind::Multi(m), TabData::Multi { rows, .. }) => {
            let changes: Vec<RowChange> = rows.iter().map(|r| RowChange { before: r.before.as_ref(), after: &r.after, deleted: r.deleted }).collect();
            sql::multi_row_full(m.table, &(m.parent)(id), &changes, &opts, m.auto_id)
        }
        _ => String::new(),
    }
}

/// Requête de recherche d'un éditeur (filtres déjà réduits aux valeurs saisies).
pub fn search_sql(entity: &Entity, filters: &[(usize, String)], page: usize, page_size: usize) -> (String, Vec<Value>) {
    let s = &entity.search;
    let mut conds = Vec::new();
    let mut params = Vec::new();
    for (i, v) in filters {
        let filter = &s.filters[*i];
        match filter.op {
            crate::model::Op::Like => {
                conds.push(format!("{} LIKE ?", filter.expr));
                params.push(Value::String(format!("%{v}%")));
            }
            crate::model::Op::Eq => {
                conds.push(format!("{} = ?", filter.expr));
                params.push(v.trim().parse::<i64>().map(Value::from).unwrap_or_else(|_| Value::String(v.clone())));
            }
        }
    }
    let cols = s.columns.iter().map(|c| format!("{} AS {}", c.expr, sql::ident(c.name))).collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT {cols} FROM {}{}{} ORDER BY {} LIMIT {} OFFSET {}",
        s.from,
        if conds.is_empty() { String::new() } else { format!(" WHERE {}", conds.join(" AND ")) },
        s.group_by.map(|g| format!(" GROUP BY {g}")).unwrap_or_default(),
        s.order,
        page_size + 1,
        page * page_size
    );
    (sql, params)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::f;

    fn test_db() -> crate::db::Db {
        let url = std::env::var("ARCHFORGE_TEST_DB").expect("ARCHFORGE_TEST_DB");
        let (cred, rest) = url.split_once('@').unwrap();
        let (user, password) = cred.split_once(':').unwrap();
        let (host_port, world) = rest.split_once('/').unwrap();
        let (host, port) = host_port.split_once(':').unwrap();
        let prefix = world.trim_end_matches("_world");
        crate::db::Db::connect(crate::db::Settings {
            host: host.into(),
            port: port.parse().unwrap(),
            user: user.into(),
            password: password.into(),
            auth: format!("{prefix}_auth"),
            characters: "archcore_characters".into(),
            world: world.into(),
            logs: "archcore_logs".into(),
        })
        .unwrap()
    }

    /// Écriture réelle, sur une copie de travail seulement :
    /// `ARCHFORGE_TEST_DB=root:motdepasse@127.0.0.1:3306/archforge_dev_world cargo test ecriture -- --ignored`.
    #[test]
    #[ignore]
    fn ecriture_sur_une_base_de_travail() {
        let db = test_db();
        assert!(db.settings.world.starts_with("archforge_dev"), "test d'écriture réservé aux bases archforge_dev_*");
        let entity = &crate::defs::ENTITIES[crate::defs::by_route("creature").unwrap()];
        let id = Value::from(459);
        let data_tab = entity.tabs.iter().position(|t| t.id == "data").unwrap();
        let loot_tab = entity.tabs.iter().position(|t| t.id == "loot").unwrap();
        let mut tabs = load(&db, entity, &id, false, None).unwrap();
        let hp_before = match &tabs[data_tab] {
            TabData::Single { after: Some(r), .. } => r["data"]["MAX_HP"].clone(),
            _ => panic!(),
        };
        // Modification d'une clé JSON et ajout d'une ligne de butin, dans une seule transaction.
        if let TabData::Single { after: Some(r), .. } = &mut tabs[data_tab] {
            r.get_mut("data").unwrap().as_object_mut().unwrap().insert("MAX_HP".into(), Value::from("4242"));
        }
        if let (TabKind::Multi(m), TabData::Multi { rows, .. }) = (&entity.tabs[loot_tab].kind, &mut tabs[loot_tab]) {
            let mut after = add_row(m, &id, rows);
            after.insert("item_tid".into(), Value::from(964));
            rows.push(RowState { before: None, after, deleted: false });
        }
        let sql = [data_tab, loot_tab].map(|i| diff_sql(&entity.tabs[i], &tabs[i], &id)).join("\n");
        db.script("world", &sql, true).unwrap();
        let reloaded = load(&db, entity, &id, false, None).unwrap();
        match &reloaded[data_tab] {
            TabData::Single { after: Some(r), .. } => assert_eq!(r["data"]["MAX_HP"], Value::from("4242")),
            _ => panic!(),
        }
        let added = match &reloaded[loot_tab] {
            TabData::Multi { rows, .. } => rows.iter().find(|r| r.after["item_tid"] == Value::from(964)).cloned().expect("ligne de butin ajoutée"),
            _ => panic!(),
        };
        for (i, d) in reloaded.iter().enumerate() {
            assert_eq!(diff_sql(&entity.tabs[i], d, &id), "", "relu sans changement");
        }
        // Remise en état.
        let key = added.after["stable_key"].as_str().unwrap().to_string();
        db.script(
            "world",
            &format!(
                "UPDATE character_data_table SET data = JSON_SET(data, '$.\"MAX_HP\"', {}) WHERE tid = 459;\nDELETE FROM monster_drop WHERE stable_key = {};",
                sql::lit(&hp_before),
                sql::lit(&Value::String(key))
            ),
            true,
        )
        .unwrap();
        // Une erreur au milieu d'un script n'écrit rien.
        let bad = "UPDATE character_data_table SET name = 'X' WHERE tid = 459;\nUPDATE character_data_table SET nope = 1 WHERE tid = 459;";
        assert!(db.script("world", bad, true).is_err());
        let name = db.select_maps("world", "SELECT name FROM character_data_table WHERE tid = 459", &[]).unwrap();
        assert_eq!(name[0]["name"], Value::from("Young Hydra"), "transaction annulée");
    }

    /// Sur une vraie base ArchCore : `ARCHFORGE_TEST_DB=root:motdepasse@127.0.0.1:3306/archcore_world cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn tous_les_editeurs_sur_une_vraie_base() {
        let db = test_db();
        for entity in crate::defs::ENTITIES.iter() {
            let (q, params) = search_sql(entity, &[], 0, 5);
            let found = db.select(entity.db, &q, &params).unwrap_or_else(|e| panic!("{} : recherche : {e:#}\n{q}", entity.route));
            for (i, filter) in entity.search.filters.iter().enumerate() {
                let (q, params) = search_sql(entity, &[(i, "1".into())], 0, 5);
                db.select(entity.db, &q, &params).unwrap_or_else(|e| panic!("{} : filtre {} : {e:#}\n{q}", entity.route, filter.name));
            }
            let Some(first) = found.maps().into_iter().next() else {
                eprintln!("{} : table vide", entity.route);
                continue;
            };
            let id = first.get(entity.key_col).cloned().unwrap();
            let tabs = load(&db, entity, &id, false, None).unwrap_or_else(|e| panic!("{} {id} : chargement : {e:#}", entity.route));
            for (tab, data) in entity.tabs.iter().zip(&tabs) {
                assert_eq!(diff_sql(tab, data, &id), "", "{} {id} / {} : changement sans modification", entity.route, tab.id);
                let _ = full_sql(tab, data, &id);
            }
            if let Some(kind) = entity.reference {
                let r = crate::model::ref_def(kind);
                let group = if r.grouped { format!(" GROUP BY {}", r.key) } else { String::new() };
                let q = format!("SELECT {} AS k, {} AS n FROM {} WHERE {} IN (?){group}", r.key, r.name, r.from, r.key);
                db.select(r.db, &q, &[id.clone()]).unwrap_or_else(|e| panic!("{} : référence : {e:#}\n{q}", entity.route));
            }
            // Création par copie : rien ne doit manquer pour écrire la nouvelle entrée.
            if !matches!(entity.create, crate::model::Create::No) {
                let tabs = load(&db, entity, &id, true, Some(&id)).unwrap_or_else(|e| panic!("{} : copie : {e:#}", entity.route));
                for (tab, data) in entity.tabs.iter().zip(&tabs) {
                    let _ = diff_sql(tab, data, &id);
                }
            }
            eprintln!("{} : {id} ok ({} onglets)", entity.route, tabs.len());
        }
    }

    #[test]
    fn saisie_typee() {
        let n = f("a", "A").int();
        assert_eq!(parse_input("42", &n), Value::from(42));
        assert_eq!(parse_input("-", &n), Value::from("-"));
        assert!(!is_valid(&parse_input("-", &n), &n));
        let x = f("x", "X").float();
        assert_eq!(parse_input("1.5", &x), Value::from(1.5));
        assert_eq!(parse_input("1.", &x), Value::from("1."), "saisie en cours : reste du texte");
        assert!(is_valid(&parse_input("1.", &x), &x));
        let t = f("t", "T").nullable();
        assert_eq!(parse_input("", &t), Value::Null);
    }

    #[test]
    fn etat_des_lignes() {
        let r = crate::model::row(&[("a", Value::from(1))]);
        let mut s = RowState { before: Some(r.clone()), after: r.clone(), deleted: false };
        assert!(s.status() == RowStatus::Same);
        s.after.insert("a".into(), Value::from("1"));
        assert!(s.status() == RowStatus::Same, "1 et « 1 » : inchangé");
        s.after.insert("a".into(), Value::from(2));
        assert!(s.status() == RowStatus::Changed);
        s.deleted = true;
        assert!(s.status() == RowStatus::Deleted);
    }
}
