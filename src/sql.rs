//! Génération du SQL de l'éditeur.
//!
//! Deux formes, comme Keira3 :
//!   - « changements » : seulement ce qui diffère de la base (UPDATE ciblé, JSON_SET / JSON_REMOVE pour les
//!     colonnes JSON, DELETE et INSERT des lignes retirées ou ajoutées) ;
//!   - « entrée complète » : DELETE puis INSERT de toute l'entrée, pour une sauvegarde.
//!
//! Toutes les requêtes sont rejouables, comme l'exige la couche de contenu d'ArchCore.

use serde_json::{Map, Value};

pub type Row = Map<String, Value>;

/// Condition sur une colonne : égalité, ou appartenance à une liste (textes de plusieurs sortes d'une entrée).
#[derive(Clone, Debug, PartialEq)]
pub enum Cond {
    Eq(Value),
    In(Vec<Value>),
}

pub type Key = Vec<(String, Cond)>;

#[cfg(test)]
pub fn key_eq(pairs: &[(&str, Value)]) -> Key {
    pairs.iter().map(|(c, v)| (c.to_string(), Cond::Eq(v.clone()))).collect()
}

pub fn ident(name: &str) -> String {
    format!("`{}`", name.replace('`', "``"))
}

pub fn lit(value: &Value) -> String {
    match value {
        Value::Null => "NULL".into(),
        Value::Bool(b) => if *b { "1" } else { "0" }.into(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => quote(s),
        other => quote(&other.to_string()),
    }
}

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        match c {
            '\0' => out.push_str("\\0"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\x1a' => out.push_str("\\Z"),
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            c => out.push(c),
        }
    }
    out.push('\'');
    out
}

pub fn where_clause(key: &[(String, Cond)]) -> String {
    key.iter()
        .map(|(col, cond)| match cond {
            Cond::Eq(Value::Null) => format!("{} IS NULL", ident(col)),
            Cond::Eq(v) => format!("{} = {}", ident(col), lit(v)),
            Cond::In(vs) => format!("{} IN ({})", ident(col), vs.iter().map(lit).collect::<Vec<_>>().join(", ")),
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

/// Égalité de deux valeurs ; 5 et « 5 » sont égaux pour la base : une saisie ne change pas une valeur qu'on
/// n'a pas modifiée.
pub fn deep_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(n), Value::String(s)) | (Value::String(s), Value::Number(n)) => n.to_string() == *s,
        (Value::Number(x), Value::Number(y)) => x == y || x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| deep_equal(p, q)),
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| deep_equal(v, w)))
        }
        _ => a == b,
    }
}

#[derive(Clone, Debug)]
enum PathPart {
    Key(String),
    Index(usize),
}

/// Chemin JSON de MariaDB : $."clé avec espace"[3]
fn json_path(parts: &[PathPart]) -> String {
    let mut out = String::from("$");
    for p in parts {
        match p {
            PathPart::Index(i) => out.push_str(&format!("[{i}]")),
            PathPart::Key(k) => out.push_str(&format!(".\"{}\"", k.replace('\\', "\\\\").replace('"', "\\\""))),
        }
    }
    out
}

/// Un objet ou une liste s'insère comme JSON, pas comme une chaîne.
fn json_value(v: &Value) -> String {
    if v.is_object() || v.is_array() {
        format!("JSON_EXTRACT({}, '$')", lit(v))
    } else {
        lit(v)
    }
}

#[derive(Default)]
struct Ops {
    set: Vec<(Vec<PathPart>, Value)>,
    remove: Vec<Vec<PathPart>>,
}

fn diff_json(path: Vec<PathPart>, a: &Value, b: &Value, ops: &mut Ops) {
    if deep_equal(a, b) {
        return;
    }
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for k in x.keys() {
                if !y.contains_key(k) {
                    let mut p = path.clone();
                    p.push(PathPart::Key(k.clone()));
                    ops.remove.push(p);
                }
            }
            for (k, v) in y {
                let mut p = path.clone();
                p.push(PathPart::Key(k.clone()));
                match x.get(k) {
                    None => ops.set.push((p, v.clone())),
                    Some(old) => diff_json(p, old, v, ops),
                }
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() && path.len() < 4 => {
            for (i, (old, new)) in x.iter().zip(y).enumerate() {
                let mut p = path.clone();
                p.push(PathPart::Index(i));
                diff_json(p, old, new, ops);
            }
        }
        _ => ops.set.push((path, b.clone())),
    }
}

/// Expression SQL qui fait passer la colonne JSON `col` de `before` à `after`, ou None si rien ne change.
/// Les clés inchangées ne sont pas recopiées : un fichier de contenu ne reprend pas les données du client.
pub fn json_patch(col: &str, before: &Value, after: &Value) -> Option<String> {
    const MAX_OPS: usize = 40;
    if deep_equal(before, after) {
        return None;
    }
    let whole = || Some(lit(&Value::String(after.to_string())));
    if before.is_null() || after.is_null() || !(before.is_object() || before.is_array()) {
        return if after.is_null() { Some("NULL".into()) } else { whole() };
    }
    let mut ops = Ops::default();
    diff_json(Vec::new(), before, after, &mut ops);
    if ops.set.iter().any(|(p, _)| p.is_empty()) || ops.set.len() + ops.remove.len() > MAX_OPS {
        return whole();
    }
    let mut expr = ident(col);
    if !ops.remove.is_empty() {
        let paths: Vec<String> = ops.remove.iter().map(|p| quote(&json_path(p))).collect();
        expr = format!("JSON_REMOVE({expr}, {})", paths.join(", "));
    }
    if !ops.set.is_empty() {
        let sets: Vec<String> = ops.set.iter().map(|(p, v)| format!("{}, {}", quote(&json_path(p)), json_value(v))).collect();
        expr = format!("JSON_SET({expr}, {})", sets.join(", "));
    }
    Some(expr)
}

/// Valeur à écrire pour une colonne (les colonnes JSON sont gardées en objets dans l'éditeur).
fn cell(row: &Row, col: &str, json: &[&str]) -> String {
    match row.get(col) {
        None | Some(Value::Null) => "NULL".into(),
        Some(v) if json.contains(&col) && !v.is_string() => quote(&v.to_string()),
        Some(v) => lit(v),
    }
}

/// Options communes : colonnes à écrire (toutes celles de la ligne sinon) et colonnes JSON.
#[derive(Clone, Copy, Default)]
pub struct Opts<'a> {
    pub columns: Option<&'a [String]>,
    pub json: &'a [&'a str],
}

fn columns_of(row: &Row, opts: &Opts) -> Vec<String> {
    match opts.columns {
        Some(cols) => cols.to_vec(),
        None => row.keys().cloned().collect(),
    }
}

pub fn update_query(table: &str, key: &[(String, Cond)], before: &Row, after: &Row, opts: &Opts) -> String {
    let mut sets = Vec::new();
    for c in columns_of(after, opts) {
        let (b, a) = (before.get(&c).unwrap_or(&Value::Null), after.get(&c).unwrap_or(&Value::Null));
        if deep_equal(b, a) {
            continue;
        }
        let expr = if opts.json.contains(&c.as_str()) { json_patch(&c, b, a) } else { Some(lit(a)) };
        if let Some(e) = expr {
            sets.push(format!("{} = {e}", ident(&c)));
        }
    }
    if sets.is_empty() {
        return String::new();
    }
    format!("UPDATE {} SET {} WHERE {};", ident(table), sets.join(", "), where_clause(key))
}

pub fn insert_query(table: &str, rows: &[&Row], opts: &Opts, upsert: Option<&[String]>) -> String {
    let Some(first) = rows.first() else { return String::new() };
    let cols = columns_of(first, opts);
    let values: Vec<String> = rows
        .iter()
        .map(|r| format!("({})", cols.iter().map(|c| cell(r, c, opts.json)).collect::<Vec<_>>().join(", ")))
        .collect();
    let mut sql = format!(
        "INSERT INTO {} ({}) VALUES\n{}",
        ident(table),
        cols.iter().map(|c| ident(c)).collect::<Vec<_>>().join(", "),
        values.join(",\n")
    );
    if let Some(key_cols) = upsert {
        let updatable: Vec<String> = cols
            .iter()
            .filter(|c| !key_cols.contains(c))
            .map(|c| format!("{} = VALUES({})", ident(c), ident(c)))
            .collect();
        if !updatable.is_empty() {
            sql.push_str(&format!("\nON DUPLICATE KEY UPDATE {}", updatable.join(", ")));
        }
    }
    sql + ";"
}

/// Insertion d'une ligne dont l'identifiant (`auto_id`) n'est qu'un numéro : le prochain libre au moment où
/// la requête passe. La clé stable de la ligne, unique, rend la requête rejouable (mise à jour au 2e passage).
pub fn insert_auto_id_query(table: &str, row: &Row, auto_id: &str, opts: &Opts, key_cols: &[String]) -> String {
    let cols: Vec<String> = columns_of(row, opts).into_iter().filter(|c| c != auto_id).collect();
    let t = ident(table);
    let all: Vec<String> = std::iter::once(auto_id.to_string()).chain(cols.iter().cloned()).map(|c| ident(&c)).collect();
    let updatable: Vec<String> = cols
        .iter()
        .filter(|c| !key_cols.contains(c))
        .map(|c| format!("{} = VALUES({})", ident(c), ident(c)))
        .collect();
    format!(
        "INSERT INTO {t} ({})\nSELECT COALESCE(MAX({}), 0) + 1, {} FROM {t}\nON DUPLICATE KEY UPDATE {};",
        all.join(", "),
        ident(auto_id),
        cols.iter().map(|c| cell(row, c, opts.json)).collect::<Vec<_>>().join(", "),
        updatable.join(", ")
    )
}

pub fn delete_query(table: &str, key: &[(String, Cond)]) -> String {
    format!("DELETE FROM {} WHERE {};", ident(table), where_clause(key))
}

fn pick(row: &Row, cols: &[String]) -> Key {
    cols.iter().map(|c| (c.clone(), Cond::Eq(row.get(c).cloned().unwrap_or(Value::Null)))).collect()
}

// --- Une ligne (onglet « une ligne » : modèle, objet, quête…) ---

/// before : ligne en base (None si elle n'existe pas encore) ; key_cols : colonnes de la clé primaire.
pub fn single_row_diff(table: &str, key_cols: &[String], before: Option<&Row>, after: &Row, opts: &Opts) -> String {
    match before {
        None => insert_query(table, &[after], opts, Some(key_cols)),
        Some(b) => update_query(table, &pick(b, key_cols), b, after, opts),
    }
}

pub fn single_row_full(table: &str, key_cols: &[String], after: &Row, opts: &Opts) -> String {
    format!("{}\n{}", delete_query(table, &pick(after, key_cols)), insert_query(table, &[after], opts, None))
}

// --- Plusieurs lignes rattachées à une entrée (apparitions, butin, boutique…) ---

pub struct RowChange<'a> {
    pub before: Option<&'a Row>,
    pub after: &'a Row,
    pub deleted: bool,
}

/// row_key : colonnes qui désignent une ligne de façon stable (stable_key, ou clé primaire).
pub fn multi_row_diff(table: &str, row_key: &[String], rows: &[RowChange], opts: &Opts, auto_id: Option<&str>) -> String {
    let mut out = Vec::new();
    for r in rows {
        match (r.before, r.deleted) {
            (Some(b), true) => out.push(delete_query(table, &pick(b, row_key))),
            (None, true) => {}
            (None, false) => out.push(match auto_id {
                Some(id) => insert_auto_id_query(table, r.after, id, opts, row_key),
                None => insert_query(table, &[r.after], opts, Some(row_key)),
            }),
            (Some(b), false) => {
                let q = update_query(table, &pick(b, row_key), b, r.after, opts);
                if !q.is_empty() {
                    out.push(q);
                }
            }
        }
    }
    out.join("\n")
}

pub fn multi_row_full(table: &str, parent: &[(String, Cond)], rows: &[RowChange], opts: &Opts, auto_id: Option<&str>) -> String {
    let kept: Vec<&RowChange> = rows.iter().filter(|r| !r.deleted).collect();
    let existing: Vec<&Row> = kept.iter().filter(|r| r.before.is_some() || auto_id.is_none()).map(|r| r.after).collect();
    let mut out = vec![delete_query(table, parent)];
    if !existing.is_empty() {
        out.push(insert_query(table, &existing, opts, None));
    }
    if let Some(id) = auto_id {
        for r in kept.iter().filter(|r| r.before.is_none()) {
            out.push(insert_auto_id_query(table, r.after, id, opts, &[]));
        }
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(v: Value) -> Row {
        v.as_object().unwrap().clone()
    }

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn litteraux_et_identifiants() {
        assert_eq!(lit(&Value::Null), "NULL");
        assert_eq!(lit(&json!(42)), "42");
        assert_eq!(lit(&json!(-1.5)), "-1.5");
        assert_eq!(lit(&json!("l'arc\\court\n")), "'l\\'arc\\\\court\\n'");
        assert_eq!(lit(&json!({"a": "1"})), "'{\"a\":\"1\"}'");
        assert_eq!(ident("rank"), "`rank`");
        assert_eq!(ident("a`b"), "`a``b`");
        assert_eq!(where_clause(&key_eq(&[("tid", json!(5)), ("name", json!("x"))])), "`tid` = 5 AND `name` = 'x'");
        assert_eq!(where_clause(&key_eq(&[("deleted_at", Value::Null)])), "`deleted_at` IS NULL");
        let k = vec![("kind".to_string(), Cond::In(vec![json!("item_name"), json!("item_desc")]))];
        assert_eq!(where_clause(&k), "`kind` IN ('item_name', 'item_desc')");
    }

    #[test]
    fn egalite_nombre_et_texte() {
        assert!(deep_equal(&json!(5), &json!("5")));
        assert!(deep_equal(&json!({"a": [1, {"b": "2"}]}), &json!({"a": [1, {"b": 2}]})));
        assert!(!deep_equal(&json!({"a": 1}), &json!({"a": 1, "b": 2})));
        assert!(!deep_equal(&json!([1, 2]), &json!({"0": 1, "1": 2})));
    }

    #[test]
    fn patch_json_des_seules_cles_modifiees() {
        let before = json!({"NPC Price": "99", "Stack": "", "DUR": "93"});
        let after = json!({"NPC Price": "120", "DUR": "93", "Nouvelle": "1"});
        assert_eq!(
            json_patch("raw", &before, &after).unwrap(),
            "JSON_SET(JSON_REMOVE(`raw`, '$.\"Stack\"'), '$.\"NPC Price\"', '120', '$.\"Nouvelle\"', '1')"
        );
        assert_eq!(json_patch("raw", &before, &before.clone()), None);
        assert_eq!(json_patch("raw", &Value::Null, &json!({"a": "1"})).unwrap(), "'{\"a\":\"1\"}'");
    }

    #[test]
    fn patch_json_des_paires_de_npc_ini() {
        let before = json!([["Name", "Darryl"], ["TID", "584"], ["Template", "104"]]);
        let after = json!([["Name", "Darryl"], ["TID", "584"], ["Template", "105"]]);
        assert_eq!(json_patch("raw", &before, &after).unwrap(), "JSON_SET(`raw`, '$[2][1]', '105')");
        let longer = json!([["Name", "Darryl"], ["TID", "584"], ["Template", "105"], ["Function", "9"]]);
        assert_eq!(json_patch("raw", &before, &longer).unwrap(), lit(&json!(longer.to_string())));
    }

    #[test]
    fn patch_json_d_une_liste() {
        assert_eq!(json_patch("give", &json!([1, 2]), &json!([1, 2, 3])).unwrap(), "'[1,2,3]'");
        assert_eq!(
            json_patch("data", &json!({"give": [1]}), &json!({"give": [1, 2]})).unwrap(),
            "JSON_SET(`data`, '$.\"give\"', JSON_EXTRACT('[1,2]', '$'))"
        );
    }

    #[test]
    fn update_des_colonnes_changees() {
        let before = row(json!({"tid": 1, "name": "Short Bow", "stack": 1, "raw": {"Stack": ""}}));
        let after = row(json!({"tid": 1, "name": "Arc court", "stack": 1, "raw": {"Stack": "5"}}));
        let opts = Opts { json: &["raw"], ..Default::default() };
        assert_eq!(
            update_query("item_data", &key_eq(&[("tid", json!(1))]), &before, &after, &opts),
            "UPDATE `item_data` SET `name` = 'Arc court', `raw` = JSON_SET(`raw`, '$.\"Stack\"', '5') WHERE `tid` = 1;"
        );
        assert_eq!(update_query("item_data", &key_eq(&[("tid", json!(1))]), &before, &before, &opts), "");
    }

    #[test]
    fn une_ligne_creation_modification_entree_complete() {
        let r = row(json!({"tid": 9001, "name": "Épée", "raw": {"a": "1"}}));
        let opts = Opts { json: &["raw"], ..Default::default() };
        let key = s(&["tid"]);
        assert_eq!(
            single_row_diff("item_data", &key, None, &r, &opts),
            "INSERT INTO `item_data` (`tid`, `name`, `raw`) VALUES\n(9001, 'Épée', '{\"a\":\"1\"}')\nON DUPLICATE KEY UPDATE `name` = VALUES(`name`), `raw` = VALUES(`raw`);"
        );
        assert_eq!(
            single_row_full("item_data", &key, &r, &opts),
            "DELETE FROM `item_data` WHERE `tid` = 9001;\nINSERT INTO `item_data` (`tid`, `name`, `raw`) VALUES\n(9001, 'Épée', '{\"a\":\"1\"}');"
        );
        let (b, a) = (row(json!({"tid": 1, "a": 1})), row(json!({"tid": 2, "a": 1})));
        assert_eq!(single_row_diff("t", &key, Some(&b), &a, &Opts::default()), "UPDATE `t` SET `tid` = 2 WHERE `tid` = 1;");
    }

    #[test]
    fn plusieurs_lignes_visees_par_la_cle_stable() {
        let r1b = row(json!({"id": 1, "stable_key": "15:5018#1", "monster_tid": 15, "rate": 10}));
        let r1a = row(json!({"id": 1, "stable_key": "15:5018#1", "monster_tid": 15, "rate": 20}));
        let r2 = row(json!({"id": 2, "stable_key": "15:1856#1", "monster_tid": 15, "rate": 500}));
        let r3 = row(json!({"id": 3, "stable_key": "15:7#1", "monster_tid": 15, "rate": 1}));
        let r4 = row(json!({"id": null, "stable_key": "15:42#af1", "monster_tid": 15, "rate": 100}));
        let rows = [
            RowChange { before: Some(&r1b), after: &r1a, deleted: false },
            RowChange { before: Some(&r2), after: &r2, deleted: true },
            RowChange { before: Some(&r3), after: &r3, deleted: false },
            RowChange { before: None, after: &r4, deleted: false },
        ];
        let cols = s(&["id", "stable_key", "monster_tid", "rate"]);
        let opts = Opts { columns: Some(&cols), json: &[] };
        assert_eq!(
            multi_row_diff("monster_drop", &s(&["stable_key"]), &rows, &opts, Some("id")),
            "UPDATE `monster_drop` SET `rate` = 20 WHERE `stable_key` = '15:5018#1';\n\
             DELETE FROM `monster_drop` WHERE `stable_key` = '15:1856#1';\n\
             INSERT INTO `monster_drop` (`id`, `stable_key`, `monster_tid`, `rate`)\n\
             SELECT COALESCE(MAX(`id`), 0) + 1, '15:42#af1', 15, 100 FROM `monster_drop`\n\
             ON DUPLICATE KEY UPDATE `monster_tid` = VALUES(`monster_tid`), `rate` = VALUES(`rate`);"
        );
        let full = multi_row_full("monster_drop", &key_eq(&[("monster_tid", json!(15))]), &rows, &opts, Some("id"));
        assert!(full.starts_with(
            "DELETE FROM `monster_drop` WHERE `monster_tid` = 15;\n\
             INSERT INTO `monster_drop` (`id`, `stable_key`, `monster_tid`, `rate`) VALUES\n\
             (1, '15:5018#1', 15, 20),\n(3, '15:7#1', 15, 1);\nINSERT INTO `monster_drop` (`id`"
        ));
    }

    #[test]
    fn plusieurs_lignes_sans_identifiant_automatique() {
        let r = row(json!({"template_id": 104, "idx": 7, "tid": 1, "count": 1}));
        let rows = [RowChange { before: None, after: &r, deleted: false }];
        assert_eq!(
            multi_row_diff("npc_trade_list", &s(&["template_id", "idx"]), &rows, &Opts::default(), None),
            "INSERT INTO `npc_trade_list` (`template_id`, `idx`, `tid`, `count`) VALUES\n(104, 7, 1, 1)\n\
             ON DUPLICATE KEY UPDATE `tid` = VALUES(`tid`), `count` = VALUES(`count`);"
        );
        assert_eq!(insert_query("t", &[], &Opts::default(), None), "");
    }
}
