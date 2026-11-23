//! Objets (caractéristiques, modèle client, métamorphose, recette, où ils se vendent, qui les lâche),
//! boutiques, groupes et rangs de butin, production.

use serde_json::{json, Value};

use super::leak;
use crate::localized_tab;
use crate::model::*;

pub const ITEM_TYPE: &[(i64, &str)] = &[(0, "Équipement"), (1, "Consommable"), (2, "Divers")];

pub const ITEM_KIND: &[(i64, &str)] = &[
    (0, "Aucune (cape, non équipable)"),
    (1, "Armure"),
    (2, "Arme"),
    (4, "Bouclier"),
    (8, "Anneau"),
    (16, "Collier"),
    (32, "Monture"),
    (64, "Oréade"),
];

pub const ITEM_PART: &[(i64, &str)] = &[
    (0, "Aucun"),
    (2, "Corps"),
    (3, "Tête"),
    (4, "Bras"),
    (5, "Mains"),
    (6, "Jambes"),
    (7, "Pieds"),
    (9, "Main gauche (arc, bouclier)"),
    (10, "Main droite"),
    (11, "Anneau"),
    (13, "Collier"),
    (14, "Monture"),
    (15, "Arme de monture (lance)"),
    (16, "Apparence : corps"),
    (17, "Apparence : tête"),
    (18, "Apparence : bras"),
    (19, "Apparence : mains"),
    (20, "Apparence : jambes"),
    (21, "Apparence : pieds"),
    (31, "Oréade"),
];

const USABLE_TYPES: &str = "Consommables (ItemType 1), clé Type : 2 pierre d'esprit, 4 parchemin de voyage, 7 métamorphose, \
    8 potion de renforcement, 9 rune, 12 vitesse ou compétence, 13 retour de compétence, 14 orbe de résurrection, 15 rappel \
    de groupe, 17 boîte, 18 clé, 19 anti-drop, 21 page de banque, 22 feux d'artifice, 24 saut, 25 parchemin de chance, \
    26 surnom, 27 oubli des compétences, 28 réinitialisation des sockets, 35 option d'échange privé.";

const PRODUCT_CATEGORY: &[(i64, &str)] = &[(0, "Collecte (Hunt)"), (1, "Cuisine (Cook)"), (2, "Alchimie (Alchemy)")];

fn s(name: &'static str, label: &'static str) -> Field {
    f(name, label).int().server()
}

fn raw_sections() -> Vec<Section> {
    let elements = ["Magic", "Water", "Fire", "Earth", "Air", "Poison", "Linghtning", "Ice"];
    let mut element_keys = Vec::new();
    for e in elements {
        element_keys.push(s(leak(format!("{e}_Min_Damage")), leak(format!("{e} : dégâts min."))));
        element_keys.push(s(leak(format!("{e}_Max_Damage")), leak(format!("{e} : dégâts max."))));
        element_keys.push(s(leak(format!("{e}_Defense_Point")), leak(format!("{e} : défense"))));
    }
    vec![
        section(
            "Conditions",
            vec![
                s("Level", "Niveau requis").help("Pour un consommable : « min;max »."),
                f("Race", "Races").server().help("Races admises séparées par « ; » (1 humain, 2 orc, 3 elfe de lune, 4 Dragon Scion) ; vide : toutes."),
                f("Class", "Classes").server().help("Classes admises séparées par « ; » (1 combattant, 2 tireur, 3 scion, 4 mage)."),
                s("Gender", "Sexe").help("0 : les deux."),
                s("SuitableLevelMin", "Niveau conseillé min.").help("Sert aussi au tirage dans les groupes de butin."),
                s("SuitableLevelMax", "Niveau conseillé max."),
            ],
        ),
        section(
            "Équipement",
            vec![
                s("Part", "Emplacement").help("2 corps, 3 tête, 5 mains, 6 jambes, 7 pieds, 9 main gauche, 10 main droite, 14 monture, 17 tête Cash."),
                s("Kind", "Sorte").help("1 armure, 2 arme, 4 bouclier ; bit 0x20 : interdit aux meurtriers de palier 2."),
                s("Hand", "Mains").help("2 : deux mains."),
                s("Type", "Type").help(USABLE_TYPES),
                s("SubType", "Sous-type").help("Modèle de la monture (Part 14), sous-type d'une potion, d'un parchemin, d'une boîte ou d'une clé."),
                s("DUR", "Durabilité max.").help("Préfixe « @ » : inusable ; 0 : ne s'use pas."),
                s("Rank", "Rang"),
                s("PhysicalRank", "Rang physique").help("Ligne des tables de chance des options."),
                s("MinSocket", "Sockets min."),
                s("MaxSocket", "Sockets max."),
                s("MinOption", "Options min."),
                s("MaxOption", "Options max."),
                f("OptionTID", "Options fixes").server().help("TID d'options séparés par « ; »."),
                f("Link_id", "Liens").server().help("Liste « ; »."),
                s("Run_Buff", "Course à cheval"),
                s("Item_section_num", "Section").help("Les objets de la même section ne comptent pas dans les régions qui l'excluent (ItemSection)."),
            ],
        ),
        section(
            "Combat",
            vec![
                s("Physical_Min_Damage", "Dégâts min."),
                s("Physical_Max_Damage", "Dégâts max."),
                s("Physical_ATKRate", "Précision"),
                s("Physical_Defense_Point", "Défense"),
                s("PDR", "Esquive (bottes)"),
                s("ATK Speed", "Vitesse d'attaque"),
                s("ATK Range", "Portée"),
                s("HP", "PV").help("Potion : PV rendus ; équipement : bonus."),
                s("MP", "PM").help("Potion : PM rendus."),
                s("Heroic_Min_Damage", "Dégâts héroïques min."),
                s("Heroic_Max_Damage", "Dégâts héroïques max."),
                s("Heroic_Defense_Point", "Défense héroïque"),
                s("CON_Bonus_Point", "Bonus constitution"),
                s("STR_Bonus_Point", "Bonus force"),
                s("INT_Bonus_Point", "Bonus intelligence"),
                s("DEX_Bonus_Point", "Bonus dextérité"),
                s("Wis_Bonus_Point", "Bonus sagesse"),
            ],
        ),
        section("Éléments", element_keys),
        section(
            "Utilisation",
            vec![
                s("ItemType", "Type d'objet").help("1 : consommable."),
                f("Use skill id", "Compétence utilisée").server().help("TID (liste « ; » pour plusieurs)."),
                f("Use skill level", "Niveau de la compétence").server(),
                s("CanStopUsingItem", "Arrêt possible"),
                s("Use Interval", "Délai entre deux usages (ms)").help("2 s par défaut pour une potion."),
                s("Apply Effect Count", "Nombre d'effets"),
                s("Apply Effect Time", "Durée d'un effet (ms)"),
                f("Destination", "Destination").server().help("Parchemin de voyage : « x;y;z »."),
                s("Polymorph Id", "Forme (métamorphose)"),
                s("Polymorph Dur", "Durée de la forme (ms)").help("0 : jusqu'à annulation."),
                s("EnableOnRide", "Utilisable à cheval"),
                s("QuestItem", "Objet de quête").help("Numéro de la quête."),
                s("EventItem", "Objet d'événement"),
            ],
        ),
        section(
            "Liaison, Cash et durée",
            vec![
                s("BoundType", "Liaison").help("1 : lié dès l'obtention ; 2 : lié à l'usage."),
                s("Cash", "Objet Cash").help("2 temps de jeu, 3 expiration, 6 familier ; au-delà de 100, moins 100."),
                s("CashItemUseType", "Usage Cash").help("2 pour un objet à durée."),
                s("RemainTime", "Temps de jeu (min)"),
                s("ExpireTime", "Expiration (min)"),
                s("FirstCategory", "Catégorie (ventes)"),
                s("SecondCategory", "Sous-catégorie (ventes)"),
            ],
        ),
        section(
            "Butin et gacha",
            vec![
                f("GroupID", "Groupe de butin").reference(RefKind::DropGroup).server().help("Groupe de drop_group dans lequel l'objet peut tomber de n'importe quel monstre."),
                s("DropRank", "Rang").help("Rang de l'objet (1 à 10), tiré selon le rang de butin du monstre."),
                s("DropRate", "Poids dans le groupe"),
                s("Gacha_Type_Numer", "Type de gacha"),
                s("GachaRank", "Rang de gacha").help("1 à 4."),
                s("GachaMinLv", "Gacha : niveau min.").help("0 : sans limite."),
                s("GachaMaxLv", "Gacha : niveau max."),
                f("Spirit Type", "Type de pierre d'esprit").server(),
            ],
        ),
    ]
}

fn item() -> Entity {
    Entity {
        route: "item",
        label: "Objets",
        singular: "Objet",
        group: "Objets",
        db: "world",
        table: "item_data",
        key_col: "tid",
        key_label: "TID",
        key_text: false,
        reference: Some(RefKind::Item),
        help: "Le TID doit exister dans les fichiers du client (icône, modèle) : on modifie les objets existants ou on en reprend un sous un autre TID connu du client.",
        create: Create::NextId("SELECT COALESCE(MAX(tid), 0) + 1 AS n FROM item_data"),
        search: Search {
            from: "item_data",
            columns: vec![
                col("tid", "tid", "TID"),
                col("name", "name", "Nom"),
                col("level", "level", "Niv."),
                col_enum("item_type", "item_type", "Type", ITEM_TYPE),
                col_enum("kind", "kind", "Sorte", ITEM_KIND),
                col_enum("part", "part", "Emplacement", ITEM_PART),
                col("npc_price", "npc_price", "Prix PNJ"),
            ],
            filters: vec![
                eq("tid", "TID", "tid"),
                like("name", "Nom", "name"),
                eq_enum("item_type", "Type", "item_type", ITEM_TYPE),
                eq_enum("kind", "Sorte", "kind", ITEM_KIND),
                eq("level", "Niveau", "level"),
            ],
            order: "tid",
            group_by: None,
        },
        tabs: vec![
            single(
                "data",
                "Objet",
                "",
                SingleTab {
                    table: "item_data",
                    key: |id| by("tid", id),
                    main: true,
                    defaults: |id| {
                        row(&[
                            ("name", json!("Nouvel objet")),
                            ("level", json!(1)),
                            ("part", json!(0)),
                            ("kind", json!(0)),
                            ("item_type", json!(2)),
                            ("type", json!(0)),
                            ("npc_price", json!(0)),
                            ("pc_price", json!(0)),
                            ("stack", json!(1)),
                            ("raw", json!({ "TID": id.to_string(), "ItemName": "Nouvel objet" })),
                        ])
                    },
                    on_copy: Some(|mut r, id| {
                        if let Some(Value::Object(raw)) = r.get_mut("raw") {
                            raw.insert("TID".into(), Value::String(id.to_string()));
                        }
                        r
                    }),
                    fields: vec![
                        f("tid", "TID").int().ro().server(),
                        f("name", "Nom").server().help("Nom d'origine (anglais) ; les traductions item_name le remplacent selon la langue du joueur."),
                        f("level", "Niveau").int().help("Copie de la clé Level ; le serveur lit la clé."),
                        f("item_type", "Type").enumeration(ITEM_TYPE),
                        f("kind", "Sorte (équipement)").enumeration(ITEM_KIND),
                        f("part", "Emplacement").enumeration(ITEM_PART),
                        f("type", "Sous-type").int().help(USABLE_TYPES),
                        f("npc_price", "Prix d'achat").int().server().group("Commerce").help("Prix payé à un PNJ (« NPC Price » des clés en est la source)."),
                        f("pc_price", "Prix de revente").int().server().group("Commerce").help("Or reçu en revendant à un PNJ."),
                        f("npc_charisma_price", "Prix en charisme").int().server().group("Commerce").help("Points de charisme payés à un PNJ, en plus de l'or (boutique des Shrines)."),
                        f("npc_shrine_coin", "Prix en Shrine Coins").int().server().group("Commerce").help("Shrine Coins (objet 53000) payés à un PNJ, en plus de l'or."),
                        f("stack", "Pile max.").int().server().group("Commerce").help("Quantité maximale par case."),
                        f("raw", "Colonnes d'itemdatatable.txt").json(raw_sections()),
                    ],
                },
            ),
            single(
                "template",
                "Modèle client",
                "itemtemplateentry.ini : nom et fichier de modèle côté client. Le serveur en lit le nom quand /create vise un TID absent d'item_data.",
                SingleTab {
                    table: "item_template",
                    key: |id| by("tid", id),
                    main: false,
                    defaults: |_| row(&[("name", json!("Nouvel objet")), ("file", Value::Null), ("raw", json!({}))]),
                    on_copy: None,
                    fields: vec![
                        f("tid", "TID").int().ro(),
                        f("name", "Nom").server(),
                        f("file", "Fichier").nullable(),
                        f("raw", "Clés").json(Vec::new()),
                    ],
                },
            ),
            single(
                "transform",
                "Métamorphose",
                "ItemTransformTable.txt : bonus ajoutés aux facteurs du joueur transformé par cette potion (négatifs possibles). La forme et la durée viennent des clés « Polymorph Id » et « Polymorph Dur » de l'objet.",
                SingleTab {
                    table: "item_transform",
                    key: |id| by("item_tid", id),
                    main: false,
                    defaults: |_| row(&[("tid", json!(0)), ("level", json!(0)), ("name", json!(""))]),
                    on_copy: None,
                    fields: vec![
                        f("item_tid", "Potion").int().ro(),
                        f("tid", "Forme").reference(RefKind::Creature).help("Gardé pour mémoire."),
                        f("level", "Niveau").int(),
                        f("name", "Nom"),
                        s("movement_run", "Course"),
                        s("atk_speed", "Vitesse d'attaque"),
                        s("max_hp", "PV max."),
                        s("max_mp", "PM max."),
                        s("defense", "Défense physique"),
                        s("min_damage", "Dégâts min."),
                        s("max_damage", "Dégâts max."),
                    ],
                },
            ),
            single(
                "recipe",
                "Recette",
                "Product*.txt : recette de cuisine ou d'alchimie qui produit cet objet.",
                SingleTab {
                    table: "product_compose",
                    key: |id| by("result_tid", id),
                    main: false,
                    defaults: |_| recipe_defaults(),
                    on_copy: None,
                    fields: recipe_fields(),
                },
            ),
            list(
                "sold",
                "Vendu dans",
                "",
                ListTab {
                    query: |id| ("SELECT template_id, idx, count FROM npc_trade_list WHERE tid = ? ORDER BY template_id".into(), vec![id.clone()]),
                    columns: vec![("template_id", "Boutique", Some(RefKind::Shop)), ("idx", "Ordre", None), ("count", "Quantité", None)],
                    empty: "Aucune boutique ne vend cet objet.",
                },
            ),
            list(
                "dropped",
                "Lâché par",
                "Butin propre aux monstres (monster_drop). Le butin des groupes passe par la clé GroupID.",
                ListTab {
                    query: |id| ("SELECT monster_tid, min_count, max_count, rate FROM monster_drop WHERE item_tid = ? ORDER BY rate DESC".into(), vec![id.clone()]),
                    columns: vec![("monster_tid", "Monstre", Some(RefKind::Creature)), ("min_count", "Min.", None), ("max_count", "Max.", None), ("rate", "Taux / 100 000", None)],
                    empty: "Aucun monstre ne lâche cet objet en propre.",
                },
            ),
            localized_tab!("item_name", "item_desc"),
        ],
    }
}

fn recipe_defaults() -> crate::sql::Row {
    row(&[
        ("category", json!(1)),
        ("name", json!("Nouvelle recette")),
        ("title_tid", json!(0)),
        ("item_level", json!(1)),
        ("price", json!(0)),
        ("success_rate", json!(100)),
        ("recipe_tid", json!(0)),
        ("recipe_bonus", json!(0)),
        ("materials", json!([])),
    ])
}

fn recipe_fields() -> Vec<Field> {
    vec![
        f("result_tid", "Objet produit").reference(RefKind::Item).ro().server(),
        f("category", "Catégorie").enumeration(PRODUCT_CATEGORY).server(),
        f("name", "Nom").server(),
        f("title_tid", "Titre exigé").reference(RefKind::Title).server().help("Titre actif exigé ; 0 : aucun."),
        f("item_level", "Niveau de compétence").int().server(),
        f("price", "Prix de la recette").int().server().help("Prix d'achat de la recette au PNJ."),
        f("success_rate", "Chance de réussite").int().server(),
        f("recipe_tid", "Objet « recette »").reference(RefKind::Item).server().help("Facultatif ; 0 : aucun."),
        f("recipe_bonus", "Bonus de la recette").int().server(),
        f("materials", "Matériaux").pair_list(Some(RefKind::Item), "Objet", "Quantité").server(),
    ]
}

fn recipe() -> Entity {
    Entity {
        route: "recipe",
        label: "Recettes",
        singular: "Recette",
        group: "Production",
        db: "world",
        table: "product_compose",
        key_col: "result_tid",
        key_label: "Objet produit (TID)",
        key_text: false,
        reference: Some(RefKind::Item),
        help: "Recettes de cuisine et d'alchimie, désignées par le TID de l'objet produit.",
        create: Create::Manual,
        search: Search {
            from: "product_compose",
            columns: vec![
                col("result_tid", "result_tid", "Objet"),
                col("name", "name", "Nom"),
                col_enum("category", "category", "Catégorie", PRODUCT_CATEGORY),
                col("item_level", "item_level", "Niveau"),
                col("price", "price", "Prix"),
            ],
            filters: vec![eq("result_tid", "Objet (TID)", "result_tid"), like("name", "Nom", "name"), eq_enum("category", "Catégorie", "category", PRODUCT_CATEGORY)],
            order: "category, item_level, result_tid",
            group_by: None,
        },
        tabs: vec![single(
            "recipe",
            "Recette",
            "",
            SingleTab { table: "product_compose", key: |id| by("result_tid", id), main: true, defaults: |_| recipe_defaults(), on_copy: None, fields: recipe_fields() },
        )],
    }
}

fn product_skill() -> Entity {
    Entity {
        route: "product-skill",
        label: "Compétences de production",
        singular: "Compétence de production",
        group: "Production",
        db: "world",
        table: "product_category",
        key_col: "category",
        key_label: "Catégorie",
        key_text: false,
        reference: None,
        help: "productcategory.txt et productexp.txt : compétence de chaque catégorie et expérience par niveau (1 à 15).",
        create: Create::No,
        search: Search {
            from: "product_category",
            columns: vec![col_enum("category", "category", "Catégorie", PRODUCT_CATEGORY), col("skill_tid", "skill_tid", "Compétence"), col("skill_name", "skill_name", "Nom"), col("npc", "npc", "PNJ")],
            filters: vec![],
            order: "category",
            group_by: None,
        },
        tabs: vec![
            single(
                "category",
                "Catégorie",
                "Le prix et le niveau de la ligne ne servent pas : le client affiche et vérifie ceux de SKILL_CONST.",
                SingleTab {
                    table: "product_category",
                    key: |id| by("category", id),
                    main: true,
                    defaults: |_| row(&[]),
                    on_copy: None,
                    fields: vec![
                        f("category", "Catégorie").enumeration(PRODUCT_CATEGORY).ro().server(),
                        f("skill_tid", "Compétence").reference(RefKind::Skill).server(),
                        f("skill_name", "Nom de la compétence"),
                        f("price", "Prix").int(),
                        f("level", "Niveau").int(),
                        f("npc", "PNJ qui l'enseigne"),
                    ],
                },
            ),
            multi(
                "exp",
                "Expérience",
                "Par niveau : expérience gagnée par action (min, max), bonus de réussite, expérience du niveau et cumulée à sa fin.",
                MultiTab {
                    // Les niveaux d'expérience sont rattachés à la compétence de la catégorie.
                    table: "product_exp",
                    parent: |id| {
                        let skill = match id.as_i64() {
                            Some(0) => 663,
                            Some(1) => 203,
                            Some(2) => 285,
                            _ => 0,
                        };
                        by("skill_tid", &Value::from(skill))
                    },
                    order: "level",
                    row_key: vec!["skill_tid", "level"],
                    auto_id: None,
                    auto_increment: None,
                    copy: false,
                    columns: vec![
                        s("level", "Niveau"),
                        s("exp_min", "Expérience min."),
                        s("exp_max", "Expérience max."),
                        s("rate_bonus", "Bonus de réussite"),
                        s("level_exp", "Expérience du niveau"),
                        s("total_exp", "Cumul"),
                    ],
                    new_row: |_, rows, last| {
                        let mut r = last.cloned().unwrap_or_default();
                        let level = rows.iter().filter_map(|r| r.get("level").and_then(|v| v.as_i64())).max().unwrap_or(0) + 1;
                        r.insert("level".into(), json!(level));
                        r
                    },
                    fresh: None,
                },
            ),
        ],
    }
}

fn gather_set() -> Entity {
    Entity {
        route: "gather-set",
        label: "Ensembles de récolte",
        singular: "Ensemble de récolte",
        group: "Production",
        db: "world",
        table: "product_result_item",
        key_col: "item_set",
        key_label: "Ensemble",
        key_text: false,
        reference: Some(RefKind::ItemSet),
        help: "Objets qu'une récolte peut donner, par niveau de compétence, avec leur poids (100 par bloc). Un monstre désigne son ensemble dans l'onglet « Récolte » de la créature.",
        create: Create::NextId("SELECT COALESCE(MAX(item_set), 0) + 1 AS n FROM product_result_item"),
        search: Search {
            from: "product_result_item",
            columns: vec![
                col("item_set", "item_set", "Ensemble"),
                col("COUNT(*)", "items", "Lignes"),
                col("(SELECT COUNT(*) FROM product_gather g WHERE g.item_set = product_result_item.item_set)", "monsters", "Monstres"),
            ],
            filters: vec![eq("item_set", "Ensemble", "item_set"), eq("item_tid", "Contient l'objet (TID)", "item_tid")],
            order: "item_set",
            group_by: Some("item_set"),
        },
        tabs: vec![
            multi(
                "items",
                "Objets",
                "",
                MultiTab {
                    table: "product_result_item",
                    parent: |id| by("item_set", id),
                    order: "skill_level, item_tid",
                    row_key: vec!["item_set", "skill_level", "item_tid"],
                    auto_id: None,
                    auto_increment: None,
                    copy: true,
                    columns: vec![s("skill_level", "Niveau de compétence"), f("item_tid", "Objet").reference(RefKind::Item).server(), s("weight", "Poids")],
                    new_row: |id, _, last| {
                        let level = last.and_then(|l| l.get("skill_level").cloned()).unwrap_or(json!(1));
                        row(&[("item_set", id.clone()), ("skill_level", level), ("item_tid", json!(0)), ("weight", json!(100))])
                    },
                    fresh: None,
                },
            ),
            list(
                "monsters",
                "Monstres",
                "Monstres dont la récolte utilise cet ensemble.",
                ListTab {
                    query: |id| ("SELECT monster_tid, skill_tid, rate FROM product_gather WHERE item_set = ? ORDER BY monster_tid".into(), vec![id.clone()]),
                    columns: vec![("monster_tid", "Monstre", Some(RefKind::Creature)), ("skill_tid", "Compétence", Some(RefKind::Skill)), ("rate", "Chance", None)],
                    empty: "Aucun monstre n'utilise cet ensemble.",
                },
            ),
        ],
    }
}

fn shop() -> Entity {
    Entity {
        route: "shop",
        label: "Boutiques",
        singular: "Boutique",
        group: "Objets",
        db: "world",
        table: "npc_trade_list",
        key_col: "template_id",
        key_label: "Modèle de boutique",
        key_text: false,
        reference: Some(RefKind::Shop),
        help: "Listes d'objets vendus par les PNJ (npctradeitemlist.txt). Un PNJ y renvoie par le « Template » de sa fonction 9.",
        create: Create::NextId("SELECT COALESCE(MAX(template_id), 0) + 1 AS n FROM npc_trade_list"),
        search: Search {
            from: "npc_trade_list",
            columns: vec![
                col("template_id", "template_id", "Modèle"),
                col("COUNT(*)", "items", "Objets"),
                col("GROUP_CONCAT(tid ORDER BY idx SEPARATOR ', ')", "tids", "TID des objets"),
            ],
            filters: vec![eq("template_id", "Modèle", "template_id"), eq("tid", "Contient l'objet (TID)", "tid")],
            order: "template_id",
            group_by: Some("template_id"),
        },
        tabs: vec![
            multi(
                "items",
                "Articles",
                "Ordre d'affichage (idx), objet et quantité vendue par achat. Le prix vient de l'objet (npc_price, charisme, Shrine Coins).",
                MultiTab {
                    table: "npc_trade_list",
                    parent: |id| by("template_id", id),
                    order: "idx",
                    row_key: vec!["template_id", "idx"],
                    auto_id: None,
                    auto_increment: None,
                    copy: true,
                    columns: vec![s("idx", "Ordre"), f("tid", "Objet").reference(RefKind::Item).server(), s("count", "Quantité")],
                    new_row: |id, rows, _| {
                        let idx = rows.iter().filter_map(|r| r.get("idx").and_then(|v| v.as_i64())).max().map_or(0, |n| n + 1);
                        row(&[("template_id", id.clone()), ("idx", json!(idx)), ("tid", json!(0)), ("count", json!(1))])
                    },
                    fresh: None,
                },
            ),
            list(
                "sellers",
                "Vendeurs",
                "PNJ dont une fonction 9 désigne cette boutique.",
                ListTab {
                    // Espaces facultatifs : MariaDB en ajoute quand il réécrit le JSON (JSON_SET).
                    query: |id| {
                        (
                            r#"SELECT id, name, tid FROM npc_spawn WHERE raw REGEXP CONCAT('"NPCTradeStart", ?"0"[]], ?[[]"Template", ?"', ?, '"[]]') ORDER BY id"#.into(),
                            vec![Value::String(crate::editor::display(id))],
                        )
                    },
                    columns: vec![("id", "PNJ", Some(RefKind::Npc)), ("name", "Nom", None), ("tid", "Modèle", Some(RefKind::Creature))],
                    empty: "Aucun PNJ ne vend cette liste.",
                },
            ),
        ],
    }
}

fn drop_group() -> Entity {
    Entity {
        route: "drop-group",
        label: "Groupes de butin",
        singular: "Groupe de butin",
        group: "Objets",
        db: "world",
        table: "drop_group",
        key_col: "group_id",
        key_label: "Groupe",
        key_text: false,
        reference: Some(RefKind::DropGroup),
        help: "groupdroprate.txt : tout monstre peut lâcher un objet d'un groupe ; les objets d'un groupe sont ceux dont la clé GroupID d'item_data le désigne.",
        create: Create::NextId("SELECT COALESCE(MAX(group_id), 0) + 1 AS n FROM drop_group"),
        search: Search {
            from: "drop_group",
            columns: vec![col("group_id", "group_id", "Groupe"), col("name", "name", "Nom"), col("rate", "rate", "Taux / 100 000"), col("meditation", "meditation", "Méditation")],
            filters: vec![like("name", "Nom", "name")],
            order: "group_id",
            group_by: None,
        },
        tabs: vec![
            single(
                "group",
                "Groupe",
                "",
                SingleTab {
                    table: "drop_group",
                    key: |id| by("group_id", id),
                    main: true,
                    defaults: |_| row(&[("name", json!("Nouveau groupe")), ("meditation", json!(0)), ("rate", json!(0)), ("level_bonus", json!([0, 0, 0, 0, 0, 0, 0, 0, 0, 0]))]),
                    on_copy: None,
                    fields: vec![
                        f("group_id", "Groupe").int().ro(),
                        f("name", "Nom"),
                        s("rate", "Taux / 100 000").help("Chance de tirer un objet de ce groupe à chaque mort de monstre."),
                        f("meditation", "Méditation").bool().server().help("Taux multiplié par la clé DropMeditation du monstre."),
                        f("level_bonus", "Bonus par tranche de niveau").list(None).server().help("10 bonus ajoutés au taux (sur 100 000), un par tranche de 10 niveaux du tueur, dans l'ordre."),
                    ],
                },
            ),
            list(
                "items",
                "Objets du groupe",
                "Objets dont la clé GroupID désigne ce groupe (200 premiers). Pour en ajouter ou en retirer, modifier la clé GroupID de l'objet.",
                ListTab {
                    query: |id| {
                        (
                            "SELECT tid, name, level, JSON_VALUE(raw, '$.DropRank') AS `rank`, JSON_VALUE(raw, '$.DropRate') AS weight FROM item_data WHERE JSON_VALUE(raw, '$.GroupID') = ? ORDER BY level, tid LIMIT 200".into(),
                            vec![Value::String(crate::editor::display(id))],
                        )
                    },
                    columns: vec![("tid", "Objet", Some(RefKind::Item)), ("name", "Nom", None), ("level", "Niv.", None), ("rank", "Rang", None), ("weight", "Poids", None)],
                    empty: "Aucun objet dans ce groupe.",
                },
            ),
        ],
    }
}

fn drop_rank() -> Entity {
    Entity {
        route: "drop-rank",
        label: "Rangs de butin",
        singular: "Rang de butin",
        group: "Objets",
        db: "world",
        table: "drop_rank_rate",
        key_col: "monster_rank",
        key_label: "Rang du monstre",
        key_text: false,
        reference: None,
        help: "droprankrate.txt : pour un rang de butin de monstre (clé DropRank), chance en pour cent de tirer un objet de chaque rang 1 à 10.",
        create: Create::NextId("SELECT COALESCE(MAX(monster_rank), 0) + 1 AS n FROM drop_rank_rate"),
        search: Search {
            from: "drop_rank_rate",
            columns: vec![col("monster_rank", "monster_rank", "Rang du monstre"), col("rates", "rates", "Pour cent par rang d'objet")],
            filters: vec![],
            order: "monster_rank",
            group_by: None,
        },
        tabs: vec![single(
            "rank",
            "Rang",
            "",
            SingleTab {
                table: "drop_rank_rate",
                key: |id| by("monster_rank", id),
                main: true,
                defaults: |_| row(&[("rates", json!([0, 0, 0, 0, 0, 0, 0, 0, 0, 0]))]),
                on_copy: None,
                fields: vec![
                    f("monster_rank", "Rang du monstre").int().ro().server(),
                    f("rates", "Pour cent par rang d'objet").list(None).server().help("10 valeurs : rangs d'objet 1 à 10."),
                ],
            },
        )],
    }
}

pub fn entities() -> Vec<Entity> {

    vec![item(), shop(), drop_group(), drop_rank(), recipe(), product_skill(), gather_set()]
}
