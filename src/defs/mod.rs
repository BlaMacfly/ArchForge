//! Éditeurs d'ArchForge, un fichier par domaine. Les libellés viennent des données et du code d'ArchCore
//! (commentaires de `sql/migrations/`, lecture des tables dans `crates/archcore/src/world/`), pas d'une
//! documentation extérieure. Un champ marqué « serveur » est lu par le serveur.

mod character;
mod creature;
mod item;
mod misc;
mod quest;
mod world;

use crate::model::Entity;
use std::sync::LazyLock;

pub static ENTITIES: LazyLock<Vec<Entity>> = LazyLock::new(|| {
    let mut all = Vec::new();
    all.extend(creature::entities());
    all.extend(item::entities());
    all.extend(quest::entities());
    all.extend(character::entities());
    all.extend(world::entities());
    all.extend(misc::entities());
    all
});

pub fn by_route(route: &str) -> Option<usize> {
    ENTITIES.iter().position(|e| e.route == route)
}

/// Nom d'une fonction de PNJ (npc.ini).
pub fn npc_function(n: i64) -> Option<&'static str> {
    creature::NPC_FUNCTIONS.iter().find(|(k, _)| *k == n).map(|(_, l)| *l)
}

// ------------------------------------------------------------------ valeurs partagées

pub const RACE: &[(i64, &str)] = &[(0, "Aucune / toutes"), (1, "Humain"), (2, "Orc"), (3, "Elfe de lune"), (4, "Dragon Scion")];

pub const CLASS: &[(i64, &str)] = &[
    (0, "Aucune / toutes"),
    (1, "Combattant (Chevalier, Berserker, Swashbuckler, Slayer)"),
    (2, "Tireur (Archer, Chasseur, Ranger, Orbiter)"),
    (3, "Scion"),
    (4, "Mage (Mage, Sorcier, Élémentaliste, Summoner)"),
];

pub const GENDER: &[(i64, &str)] = &[(0, "—"), (1, "Masculin"), (2, "Féminin")];

/// Races admises (bits) : 0 = toutes.
pub const RACE_BITS: &[(i64, &str)] = &[(1, "Humain"), (2, "Orc"), (4, "Elfe de lune"), (8, "Dragon Scion")];

/// Classes admises (bit n : classe n).
pub const CLASS_BITS: &[(i64, &str)] = &[(2, "Combattant"), (4, "Tireur"), (8, "Scion"), (16, "Mage")];

pub const SAFETY: &[(i64, &str)] = &[(0, "Sûre"), (1, "Libre"), (2, "Dangereuse")];

pub const YES_NO: &[(i64, &str)] = &[(0, "non"), (1, "oui")];

/// Sortes de textes localisés (`localized_text.kind`).
pub const LOCALIZED_KINDS: &[(&str, &str)] = &[
    ("npc_name", "Nom de PNJ (npc_spawn.id)"),
    ("monster_name", "Nom de monstre (TID)"),
    ("item_name", "Nom d'objet (TID)"),
    ("item_desc", "Description d'objet (TID)"),
    ("quest_name", "Nom de quête (TID)"),
    ("quest_desc", "Description de quête (TID)"),
    ("skill_name", "Nom de compétence (TID)"),
    ("region_name", "Nom de région (id)"),
    ("npc_dialog", "Dialogue de PNJ (réservé)"),
];

pub const TEXT_SOURCES: &[(&str, &str)] = &[
    ("manual", "manual (saisi)"),
    ("import", "import (archcore i18n db)"),
    ("webzen", "webzen (import --webzen)"),
    ("archcore-traduction", "archcore-traduction (import --traductions)"),
];

/// Clé stable d'une ligne créée par ArchForge : reconnaissable (`#af…`), unique, assez courte pour la colonne.
pub fn forge_key(prefix: &str, max: usize) -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let suffix = format!("#af{}", radix36(nanos % 36u128.pow(9)));
    let keep = max.saturating_sub(suffix.len());
    let prefix: String = prefix.chars().take(keep).collect();
    format!("{prefix}{suffix}")
}

/// Nom calculé une seule fois (définitions des éditeurs) gardé pour toute la durée du programme.
pub fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn radix36(mut n: u128) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = Vec::new();
    loop {
        out.push(DIGITS[(n % 36) as usize]);
        n /= 36;
        if n == 0 {
            break;
        }
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// Onglet « Traductions » : textes `localized_text` des sortes données, pour l'entrée.
#[macro_export]
macro_rules! localized_tab {
    ($($kind:literal),+) => {
        $crate::model::multi(
            "i18n",
            "Traductions",
            "Textes servis aux clients dans leur langue (repli : langue de base, puis anglais, puis texte d'origine). \
             Les lignes « manual » survivent à `archcore i18n db` et aux imports.",
            $crate::model::MultiTab {
                table: "localized_text",
                parent: |id| {
                    let kinds: Vec<serde_json::Value> = vec![$(serde_json::Value::from($kind)),+];
                    let kind = if kinds.len() == 1 {
                        $crate::sql::Cond::Eq(kinds[0].clone())
                    } else {
                        $crate::sql::Cond::In(kinds)
                    };
                    vec![("kind".to_string(), kind), ("ref_id".to_string(), $crate::sql::Cond::Eq(id.clone()))]
                },
                order: "kind, lang",
                row_key: vec!["kind", "ref_id", "lang"],
                auto_id: None,
                auto_increment: None,
                copy: false,
                columns: vec![
                    $crate::model::f("kind", "Sorte").text_enum($crate::defs::LOCALIZED_KINDS),
                    $crate::model::f("lang", "Langue").reference($crate::model::RefKind::Language),
                    $crate::model::f("text", "Texte").server(),
                    $crate::model::f("source", "Origine").text_enum($crate::defs::TEXT_SOURCES),
                ],
                new_row: |id, _, _| {
                    let first = [$($kind),+][0];
                    $crate::model::row(&[
                        ("kind", serde_json::Value::from(first)),
                        ("ref_id", id.clone()),
                        ("lang", serde_json::Value::from("fr")),
                        ("text", serde_json::Value::from("")),
                        ("source", serde_json::Value::from("manual")),
                    ])
                },
                fresh: None,
            },
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cles_stables_d_archforge() {
        let k = forge_key("a1001_1", 48);
        assert!(k.starts_with("a1001_1#af") && k.len() <= 48);
        let long = forge_key(&"x".repeat(60), 40);
        assert_eq!(long.chars().count(), 40.min(long.chars().count()));
        assert!(long.contains("#af") && long.len() <= 40);
    }

    #[test]
    fn routes_uniques_et_onglets_valides() {
        let mut routes: Vec<&str> = ENTITIES.iter().map(|e| e.route).collect();
        routes.sort();
        let n = routes.len();
        routes.dedup();
        assert_eq!(routes.len(), n, "deux éditeurs ont la même route");
        for e in ENTITIES.iter() {
            assert!(!e.tabs.is_empty(), "{} sans onglet", e.route);
            let mains = e.tabs.iter().filter(|t| matches!(&t.kind, crate::model::TabKind::Single(s) if s.main)).count();
            assert!(mains <= 1, "{} : plusieurs onglets principaux", e.route);
        }
    }
}
