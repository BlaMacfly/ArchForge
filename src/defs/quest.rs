//! Quêtes (questtemplate.ini) et groupes de PNJ ou d'objets du monde qui les donnent (questgroup.ini).

use serde_json::{json, Value};

use super::leak;
use crate::localized_tab;
use crate::model::*;

fn s(name: &'static str, label: &'static str) -> Field {
    f(name, label).int().server()
}

fn quest_sections() -> Vec<Section> {
    let letters = ["A", "B", "C", "D", "E"];
    let mut objects = Vec::new();
    for l in letters {
        objects.push(s(leak(format!("Object{l}_ID")), leak(format!("Objet du monde {l}"))));
        objects.push(f(leak(format!("ObjectItem{l}_TID")), leak(format!("Objet ramassé {l}"))).int().reference(RefKind::Item).server());
        objects.push(s(leak(format!("ObjectItem{l}_Count")), leak(format!("Quantité {l}"))));
    }
    let mut gifts = vec![s("Gift_Exp", "Expérience"), s("Gift_Money", "Or")];
    for l in ["A", "B", "C", "D"] {
        gifts.push(f(leak(format!("Gift_Item{l}_Tid")), leak(format!("Objet {l}"))).reference(RefKind::Item).server());
        gifts.push(s(leak(format!("Gift_Item{l}_Count")), leak(format!("Quantité {l}"))));
    }
    let mut select = Vec::new();
    for n in 1..=8 {
        select.push(f(leak(format!("SelectItem{n}_Tid")), leak(format!("Au choix {n}"))).reference(RefKind::Item).server());
        select.push(s(leak(format!("SelectItem{n}_Count")), leak(format!("Quantité {n}"))));
    }
    let mut hunt = Vec::new();
    for l in ["A", "B"] {
        hunt.push(f(leak(format!("Complete_Monster{l}_Tid")), leak(format!("Monstre à tuer {l}"))).reference(RefKind::Creature).server());
        hunt.push(s(leak(format!("Complete_Monster{l}_Count")), leak(format!("Nombre {l}"))));
        hunt.push(f(leak(format!("DropItem{l}_TID")), leak(format!("Objet de quête lâché {l}"))).reference(RefKind::Item).server().help("Lâché par le monstre visé tant que la quête est en cours."));
        hunt.push(s(leak(format!("DropItem{l}_Count")), leak(format!("Quantité lâchée {l}"))));
        hunt.push(s(leak(format!("DropItem{l}_Property")), leak(format!("Chance {l} (%)"))).help("0 : 100 %."));
    }
    vec![
        section(
            "Conditions",
            vec![
                s("Need_Level_Min", "Niveau min."),
                s("Need_Level_Max", "Niveau max."),
                f("Need_Race", "Race").server().help("1 humain, 2 orc, 3 elfe de lune, 4 Dragon Scion (liste « ; ») ; vide : toutes."),
                f("Need_Class", "Classe").server().help("1 combattant, 2 tireur, 3 scion, 4 mage (liste « ; ») ; vide : toutes."),
                f("Need_PreviousQuestTid", "Quête préalable").reference(RefKind::Quest).server(),
                s("Self_Quest", "Quête personnelle").help("1 : prise et rendue depuis le journal, sans PNJ."),
                f("QuestGroupID", "Groupe").reference(RefKind::QuestGroup),
                f("Category", "Catégorie").int(),
            ],
        ),
        section(
            "Objectifs",
            [
                vec![
                    f("Complete_ItemTid", "Objet à rapporter").reference(RefKind::Item).server(),
                    s("Complete_ItemCount", "Quantité"),
                    s("Complete_Level", "Niveau à atteindre"),
                    f("Quest_TransItemTid", "Objet remis au départ").reference(RefKind::Item).server(),
                    s("Quest_TransItemCount", "Quantité remise"),
                ],
                hunt,
            ]
            .concat(),
        ),
        section("Objets à ramasser sur le monde", objects),
        section("Récompenses", gifts),
        section("Récompenses au choix", select),
        section(
            "Textes (client)",
            vec![
                f("QuestName", "Nom"),
                f("ScenarioName", "Scénario"),
                f("GiveNpcName", "PNJ donneur"),
                f("ConfirmNpcName", "PNJ de fin"),
                f("Dialogue_Start", "Dialogue de départ"),
                f("Dialogue_Progress", "Dialogue en cours"),
                f("Dialogue_Finish", "Dialogue de fin"),
                f("Complete_Summary1", "Résumé 1"),
                f("Complete_Summary2", "Résumé 2"),
            ],
        ),
    ]
}

fn quest() -> Entity {
    Entity {
        route: "quest",
        label: "Quêtes",
        singular: "Quête",
        group: "Quêtes",
        db: "world",
        table: "quest_template",
        key_col: "tid",
        key_label: "TID",
        key_text: false,
        reference: Some(RefKind::Quest),
        help: "Le client affiche les quêtes de ses propres fichiers : une quête neuve doit y avoir son TID. Les valeurs « a;b » ne comptent que par leur premier nombre.",
        create: Create::NextId("SELECT COALESCE(MAX(tid), 0) + 1 AS n FROM quest_template"),
        search: Search {
            from: "quest_template",
            columns: vec![
                col("tid", "tid", "TID"),
                col("name", "name", "Nom"),
                col("JSON_VALUE(data, '$.Need_Level_Min')", "lvl", "Niv. min."),
                col("JSON_VALUE(data, '$.GiveNpcName')", "giver", "Donnée par"),
                col("JSON_VALUE(data, '$.ScenarioName')", "scenario", "Scénario"),
            ],
            filters: vec![eq("tid", "TID", "tid"), like("name", "Nom", "name"), like("scenario", "Scénario", "JSON_VALUE(data, '$.ScenarioName')")],
            order: "tid",
            group_by: None,
        },
        tabs: vec![
            single(
                "quest",
                "Quête",
                "",
                SingleTab {
                    table: "quest_template",
                    key: |id| by("tid", id),
                    main: true,
                    defaults: |_| row(&[("name", json!("Nouvelle quête")), ("data", json!({"QuestName": "Nouvelle quête", "Category": "1", "Need_Level_Min": "1"}))]),
                    on_copy: None,
                    fields: vec![
                        f("tid", "TID").int().ro().server(),
                        f("name", "Nom").server(),
                        f("data", "Colonnes de questtemplate.ini").json(quest_sections()),
                    ],
                },
            ),
            list(
                "groups",
                "Groupes",
                "Groupes (questgroup.ini) dont les PNJ ou objets du monde proposent ou terminent la quête.",
                ListTab {
                    query: |id| {
                        let v = Value::String(crate::editor::display(id));
                        (
                            "SELECT id, name, IF(JSON_CONTAINS(give, ?), 'oui', '') AS donne, IF(JSON_CONTAINS(confirm, ?), 'oui', '') AS termine \
                             FROM quest_group WHERE JSON_CONTAINS(give, ?) OR JSON_CONTAINS(confirm, ?) ORDER BY id"
                                .into(),
                            vec![v.clone(), v.clone(), v.clone(), v],
                        )
                    },
                    columns: vec![("id", "Groupe", Some(RefKind::QuestGroup)), ("name", "Nom", None), ("donne", "Propose", None), ("termine", "Termine", None)],
                    empty: "Aucun groupe ne propose cette quête : aucun PNJ ni objet ne la donne.",
                },
            ),
            localized_tab!("quest_name", "quest_desc"),
        ],
    }
}

fn quest_group() -> Entity {
    Entity {
        route: "quest-group",
        label: "Groupes de quêtes",
        singular: "Groupe de quêtes",
        group: "Quêtes",
        db: "world",
        table: "quest_group",
        key_col: "id",
        key_label: "Identifiant",
        key_text: false,
        reference: Some(RefKind::QuestGroup),
        help: "Un PNJ (QuestGroupID de sa fonction 27) ou un objet du monde (quest_object) propose et termine les quêtes du groupe.",
        create: Create::NextId("SELECT COALESCE(MAX(id), 0) + 1 AS n FROM quest_group"),
        search: Search {
            from: "quest_group",
            columns: vec![
                col("id", "id", "ID"),
                col("name", "name", "Nom"),
                col("JSON_LENGTH(give)", "give", "Proposées"),
                col("JSON_LENGTH(confirm)", "confirm", "Terminées"),
            ],
            filters: vec![eq("id", "ID", "id"), like("name", "Nom", "name")],
            order: "id",
            group_by: None,
        },
        tabs: vec![
            single(
                "group",
                "Groupe",
                "",
                SingleTab {
                    table: "quest_group",
                    key: |id| by("id", id),
                    main: true,
                    defaults: |_| row(&[("name", json!("Nouveau groupe")), ("give", json!([])), ("confirm", json!([])), ("gather", json!([]))]),
                    on_copy: None,
                    fields: vec![
                        f("id", "ID").int().ro(),
                        f("name", "Nom"),
                        f("give", "Quêtes proposées").list(Some(RefKind::Quest)).server(),
                        f("confirm", "Quêtes terminées ici").list(Some(RefKind::Quest)).server(),
                        f("gather", "Ramassage sur l'objet du monde")
                            .pair_list(Some(RefKind::Quest), "Quête", "Rang")
                            .help("Quêtes dont un objet se ramasse sur l'objet du monde du groupe ; rang 0 = ObjectA … 4 = ObjectE. Sert à l'export du client."),
                    ],
                },
            ),
            list(
                "npcs",
                "PNJ",
                "",
                ListTab {
                    query: |id| {
                        (
                            r#"SELECT id, name, tid FROM npc_spawn WHERE raw REGEXP CONCAT('"QuestGroupID", ?"', ?, '"[]]') ORDER BY id"#.into(),
                            vec![Value::String(crate::editor::display(id))],
                        )
                    },
                    columns: vec![("id", "PNJ", Some(RefKind::Npc)), ("name", "Nom", None), ("tid", "Modèle", Some(RefKind::Creature))],
                    empty: "Aucun PNJ n'utilise ce groupe.",
                },
            ),
            list(
                "objects",
                "Objets du monde",
                "Objets des obj*.ini qui portent l'événement de quête (fonction 27) de ce groupe ; portée 1600. Ils viennent des fichiers du client.",
                ListTab {
                    query: |id| ("SELECT object_id, pos_x, pos_y, pos_z FROM quest_object WHERE quest_group = ? ORDER BY object_id".into(), vec![id.clone()]),
                    columns: vec![("object_id", "Objet", None), ("pos_x", "X", None), ("pos_y", "Y", None), ("pos_z", "Z", None)],
                    empty: "Aucun objet du monde ne porte ce groupe.",
                },
            ),
        ],
    }
}

pub fn entities() -> Vec<Entity> {
    vec![quest(), quest_group()]
}
