//! Modèle déclaratif des éditeurs : entités, onglets, champs et références entre tables, sur le modèle des
//! services de base de Keira3 (`SingleRowEditorService`, `MultiRowEditorService`). Un éditeur concret n'est
//! qu'une déclaration (voir `defs/`) ; le chargement, l'affichage et le SQL sont communs (`editor.rs`).

use crate::sql::{Cond, Key, Row};
use serde_json::Value;

/// Identifiant d'une entrée : nombre (TID, id) ou texte (nom d'un point de téléportation, code de langue).
pub type Id = Value;

// ------------------------------------------------------------------ références

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RefKind {
    Item,
    Creature,
    Quest,
    Skill,
    Npc,
    Shop,
    QuestGroup,
    Region,
    TeleportPoint,
    DropGroup,
    Language,
    Account,
    MonsterAi,
    Title,
    ItemSet,
    Castle,
}

/// Où trouver le nom d'une entrée référencée, et quel éditeur l'ouvre.
pub struct RefDef {
    pub label: &'static str,
    pub db: &'static str,
    pub from: &'static str,
    pub key: &'static str,
    pub name: &'static str,
    /// Clé tirée d'un GROUP BY (boutiques : une entrée = une liste de lignes).
    pub grouped: bool,
    /// Clé texte (nom, code) plutôt que numérique.
    pub text: bool,
    pub route: Option<&'static str>,
}

/// Nom d'une créature : celui de CharacterDataTable.txt, sauf s'il est illisible (texte coréen mal décodé à
/// l'import, reconnaissable à « 占 ») ; sinon celui du modèle.
pub const CREATURE_NAME: &str = "COALESCE(IF(d.name LIKE '%占%', NULL, d.name), ct.name)";

pub fn ref_def(kind: RefKind) -> RefDef {
    let r = |label, db, from, key, name, route| RefDef { label, db, from, key, name, grouped: false, text: false, route };
    match kind {
        RefKind::Item => r("Objet", "world", "item_data", "tid", "name", Some("item")),
        RefKind::Creature => r(
            "Créature",
            "world",
            "character_template ct LEFT JOIN character_data_table d ON d.tid = ct.tid",
            "ct.tid",
            CREATURE_NAME,
            Some("creature"),
        ),
        RefKind::Quest => r("Quête", "world", "quest_template", "tid", "name", Some("quest")),
        RefKind::Skill => r("Compétence", "world", "skill_template", "tid", "name", Some("skill")),
        RefKind::Npc => r("PNJ", "world", "npc_spawn", "id", "name", Some("npc")),
        RefKind::Shop => RefDef {
            grouped: true,
            ..r("Boutique", "world", "npc_trade_list", "template_id", "CONCAT(COUNT(*), ' objets')", Some("shop"))
        },
        RefKind::QuestGroup => r("Groupe de quêtes", "world", "quest_group", "id", "name", Some("quest-group")),
        RefKind::Region => r("Région", "world", "region_template", "id", "name", Some("region")),
        RefKind::TeleportPoint => {
            RefDef { text: true, ..r("Point de téléportation", "world", "teleport_point", "name", "name", Some("teleport")) }
        }
        RefKind::DropGroup => r("Groupe de butin", "world", "drop_group", "group_id", "name", Some("drop-group")),
        RefKind::Language => RefDef { text: true, ..r("Langue", "world", "language", "code", "native_name", Some("language")) },
        RefKind::Account => r("Compte", "auth", "account", "id", "username", Some("account")),
        RefKind::MonsterAi => r("IA de monstre", "world", "monster_ai", "tid", "name", Some("monster-ai")),
        RefKind::Title => r("Titre", "world", "title_template", "tid", "name", Some("title")),
        RefKind::ItemSet => RefDef {
            grouped: true,
            ..r("Ensemble de récolte", "world", "product_result_item", "item_set", "CONCAT(COUNT(*), ' objets')", Some("gather-set"))
        },
        RefKind::Castle => r("Château", "world", "siege_war", "id", "name", Some("castle")),
    }
}

// ------------------------------------------------------------------ champs

#[derive(Clone, Debug)]
pub enum Kind {
    Int,
    Float,
    Text,
    /// 0 / 1.
    Bool,
    Enum(&'static [(i64, &'static str)]),
    TextEnum(&'static [(&'static str, &'static str)]),
    /// Champ de bits, avec le libellé du cas « aucun bit ».
    Flags(&'static [(i64, &'static str)], &'static str),
    /// Clé d'une autre table (numérique, ou texte selon la référence).
    Ref(RefKind),
    /// Objet JSON clé → valeur (colonnes des tables du client), clés connues regroupées.
    Json(Vec<Section>),
    /// Paires [clé, valeur] ordonnées de npc.ini.
    Pairs,
    /// Liste JSON de valeurs (nombres, ou textes si `text`), éventuellement des clés d'une autre table.
    List { reference: Option<RefKind>, text: bool },
    /// Liste JSON de paires [TID, nombre] (matériaux, coûts, objets à charger…).
    PairList { reference: Option<RefKind>, left: &'static str, right: &'static str },
}

impl Kind {
    pub fn is_big(&self) -> bool {
        matches!(self, Kind::Json(_) | Kind::Pairs | Kind::List { .. } | Kind::PairList { .. })
    }
    pub fn is_json(&self) -> bool {
        matches!(self, Kind::Json(_) | Kind::Pairs | Kind::List { .. } | Kind::PairList { .. })
    }
    pub fn is_numeric(&self) -> bool {
        match self {
            Kind::Int | Kind::Bool | Kind::Enum(_) | Kind::Flags(..) => true,
            Kind::Ref(r) => !ref_def(*r).text,
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Section {
    pub title: &'static str,
    pub keys: Vec<Field>,
}

#[derive(Clone, Debug)]
pub struct Field {
    pub name: &'static str,
    pub label: &'static str,
    /// Explication affichée au survol ; vide : le commentaire de la colonne dans le schéma d'ArchCore.
    pub help: &'static str,
    pub kind: Kind,
    /// Lu par le serveur ArchCore (les autres ne servent qu'au client ou à l'import).
    pub server: bool,
    pub readonly: bool,
    pub nullable: bool,
    /// Vide permis : les tables du client laissent vides les colonnes sans valeur.
    pub allow_empty: bool,
    pub group: Option<&'static str>,
}

/// Champ : `f("tid", "TID").int().ro()`…
pub fn f(name: &'static str, label: &'static str) -> Field {
    Field { name, label, help: "", kind: Kind::Text, server: false, readonly: false, nullable: false, allow_empty: false, group: None }
}

impl Field {
    pub fn int(mut self) -> Self {
        self.kind = Kind::Int;
        self
    }
    pub fn float(mut self) -> Self {
        self.kind = Kind::Float;
        self
    }
    pub fn bool(mut self) -> Self {
        self.kind = Kind::Bool;
        self
    }
    pub fn enumeration(mut self, values: &'static [(i64, &'static str)]) -> Self {
        self.kind = Kind::Enum(values);
        self
    }
    pub fn text_enum(mut self, values: &'static [(&'static str, &'static str)]) -> Self {
        self.kind = Kind::TextEnum(values);
        self
    }
    pub fn flags(mut self, bits: &'static [(i64, &'static str)], none: &'static str) -> Self {
        self.kind = Kind::Flags(bits, none);
        self
    }
    pub fn reference(mut self, kind: RefKind) -> Self {
        self.kind = Kind::Ref(kind);
        self
    }
    pub fn json(mut self, sections: Vec<Section>) -> Self {
        self.kind = Kind::Json(sections);
        self
    }
    pub fn pairs(mut self) -> Self {
        self.kind = Kind::Pairs;
        self
    }
    pub fn list(mut self, reference: Option<RefKind>) -> Self {
        self.kind = Kind::List { reference, text: false };
        self
    }
    pub fn text_list(mut self) -> Self {
        self.kind = Kind::List { reference: None, text: true };
        self
    }
    pub fn pair_list(mut self, reference: Option<RefKind>, left: &'static str, right: &'static str) -> Self {
        self.kind = Kind::PairList { reference, left, right };
        self
    }
    pub fn help(mut self, help: &'static str) -> Self {
        self.help = help;
        self
    }
    pub fn server(mut self) -> Self {
        self.server = true;
        self
    }
    pub fn ro(mut self) -> Self {
        self.readonly = true;
        self
    }
    pub fn nullable(mut self) -> Self {
        self.nullable = true;
        self
    }
    pub fn group(mut self, group: &'static str) -> Self {
        self.group = Some(group);
        self
    }
}

/// Section de clés JSON.
pub fn section(title: &'static str, keys: Vec<Field>) -> Section {
    Section { title, keys }
}

// ------------------------------------------------------------------ onglets

pub type KeyFn = fn(&Id) -> Key;
pub type RowFn = fn(&Id) -> Row;

pub struct SingleTab {
    pub table: &'static str,
    pub key: KeyFn,
    /// Onglet principal : sa ligne est créée avec l'entrée.
    pub main: bool,
    pub defaults: RowFn,
    /// Ligne copiée d'une autre entrée : ajustements (TID recopié dans les clés JSON…).
    pub on_copy: Option<fn(Row, &Id) -> Row>,
    pub fields: Vec<Field>,
}

pub struct MultiTab {
    pub table: &'static str,
    /// Lignes rattachées à l'entrée.
    pub parent: KeyFn,
    pub order: &'static str,
    /// Colonnes qui désignent une ligne de façon stable (stable_key, ou clé primaire).
    pub row_key: Vec<&'static str>,
    /// Identifiant qui n'est qu'un numéro : calculé à l'insertion (MAX + 1).
    pub auto_id: Option<&'static str>,
    /// Identifiant AUTO_INCREMENT : omis à l'insertion.
    pub auto_increment: Option<&'static str>,
    /// Lignes recopiées quand on crée une entrée par copie.
    pub copy: bool,
    pub columns: Vec<Field>,
    /// Nouvelle ligne : identifiant de l'entrée, lignes actuelles, dernière ligne.
    pub new_row: fn(&Id, &[Row], Option<&Row>) -> Row,
    /// Clés régénérées pour une ligne neuve (clé stable…).
    pub fresh: Option<fn(Row) -> Row>,
}

pub struct ListTab {
    pub query: fn(&Id) -> (String, Vec<Value>),
    pub columns: Vec<(&'static str, &'static str, Option<RefKind>)>,
    pub empty: &'static str,
}

pub enum TabKind {
    Single(SingleTab),
    Multi(MultiTab),
    List(ListTab),
}

pub struct Tab {
    pub id: &'static str,
    pub label: &'static str,
    pub help: &'static str,
    /// Base de l'onglet, si elle diffère de celle de l'entité.
    pub db: Option<&'static str>,
    pub kind: TabKind,
}

impl Tab {
    pub fn table(&self) -> Option<&'static str> {
        match &self.kind {
            TabKind::Single(s) => Some(s.table),
            TabKind::Multi(m) => Some(m.table),
            TabKind::List(_) => None,
        }
    }
    pub fn json_columns(&self) -> Vec<&'static str> {
        let fields = match &self.kind {
            TabKind::Single(s) => &s.fields,
            TabKind::Multi(m) => &m.columns,
            TabKind::List(_) => return Vec::new(),
        };
        fields.iter().filter(|f| f.kind.is_json()).map(|f| f.name).collect()
    }
}

pub fn single(id: &'static str, label: &'static str, help: &'static str, s: SingleTab) -> Tab {
    Tab { id, label, help, db: None, kind: TabKind::Single(s) }
}

pub fn multi(id: &'static str, label: &'static str, help: &'static str, m: MultiTab) -> Tab {
    Tab { id, label, help, db: None, kind: TabKind::Multi(m) }
}

pub fn list(id: &'static str, label: &'static str, help: &'static str, l: ListTab) -> Tab {
    Tab { id, label, help, db: None, kind: TabKind::List(l) }
}

// ------------------------------------------------------------------ entités

pub struct SearchColumn {
    pub expr: &'static str,
    pub name: &'static str,
    pub label: &'static str,
    pub values: Option<&'static [(i64, &'static str)]>,
}

pub fn col(expr: &'static str, name: &'static str, label: &'static str) -> SearchColumn {
    SearchColumn { expr, name, label, values: None }
}

pub fn col_enum(expr: &'static str, name: &'static str, label: &'static str, values: &'static [(i64, &'static str)]) -> SearchColumn {
    SearchColumn { expr, name, label, values: Some(values) }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Op {
    Eq,
    Like,
}

pub struct Filter {
    pub name: &'static str,
    pub label: &'static str,
    pub expr: &'static str,
    pub op: Op,
    pub values: Option<&'static [(i64, &'static str)]>,
}

pub fn eq(name: &'static str, label: &'static str, expr: &'static str) -> Filter {
    Filter { name, label, expr, op: Op::Eq, values: None }
}

pub fn like(name: &'static str, label: &'static str, expr: &'static str) -> Filter {
    Filter { name, label, expr, op: Op::Like, values: None }
}

pub fn eq_enum(name: &'static str, label: &'static str, expr: &'static str, values: &'static [(i64, &'static str)]) -> Filter {
    Filter { name, label, expr, op: Op::Eq, values: Some(values) }
}

pub struct Search {
    pub from: &'static str,
    pub columns: Vec<SearchColumn>,
    pub filters: Vec<Filter>,
    pub order: &'static str,
    pub group_by: Option<&'static str>,
}

pub enum Create {
    /// Pas de création depuis l'éditeur (comptes : `archcore db-init --account`).
    No,
    /// Identifiant libre proposé par une requête (colonne `n`).
    NextId(&'static str),
    /// Identifiant saisi (texte).
    Manual,
}

pub struct Entity {
    pub route: &'static str,
    pub label: &'static str,
    pub singular: &'static str,
    pub group: &'static str,
    pub db: &'static str,
    pub table: &'static str,
    pub key_col: &'static str,
    pub key_label: &'static str,
    pub key_text: bool,
    pub reference: Option<RefKind>,
    pub help: &'static str,
    pub create: Create,
    pub search: Search,
    pub tabs: Vec<Tab>,
}

impl Entity {
    pub fn tab_db(&self, tab: &Tab) -> &'static str {
        tab.db.unwrap_or(self.db)
    }

    /// Identifiant saisi → valeur typée (nombre pour une clé numérique).
    pub fn parse_id(&self, text: &str) -> Option<Id> {
        let t = text.trim();
        if t.is_empty() {
            return None;
        }
        if self.key_text {
            return Some(Value::String(t.to_string()));
        }
        t.parse::<i64>().ok().map(Value::from)
    }
}

/// Clé `colonne = identifiant`.
pub fn by(col: &'static str, id: &Id) -> Key {
    vec![(col.to_string(), Cond::Eq(id.clone()))]
}

/// Ligne JSON à partir de paires.
pub fn row(pairs: &[(&str, Value)]) -> Row {
    pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}
