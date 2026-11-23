//! Créatures (modèle, caractéristiques serveur, apparitions, butin, récolte, croissance, apparence, boss),
//! PNJ et IA des monstres.

use serde_json::{json, Value};

use super::{forge_key, leak, CLASS, CLASS_BITS, GENDER, RACE};
use crate::localized_tab;
use crate::model::*;

const ELEMENTS: [&str; 8] = ["Magic", "Water", "Fire", "Earth", "Air", "Poison", "Thunder", "Ice"];

/// Bits de la clé `Type` des caractéristiques serveur (monsters.rs).
const MONSTER_TYPE_BITS: &[(i64, &str)] = &[
    (0x4, "Monstre"),
    (0x8, "Créature d'ambiance ou animal"),
    (0x10, "Garde"),
    (0x1_0000, "Attaquable"),
    (0x2_0000, "Ciblable"),
];

const SIEGE_CHAR_TYPE: &[(i64, &str)] = &[
    (0, "—"),
    (1, "Tour de garde"),
    (2, "Porte intérieure"),
    (3, "Porte extérieure"),
    (4, "Tour de vie"),
    (5, "Tour de résurrection"),
    (6, "Trône"),
    (7, "Œil d'Archon"),
    (8, "Protecteur"),
    (9, "Camp"),
    (10, "Catapulte"),
    (11, "Gardiens (non pris en charge)"),
];

const BOSS_CATEGORY: &[(i64, &str)] = &[
    (0, "Inconnue"),
    (1, "Chaotic Frontier"),
    (2, "Asylum"),
    (3, "Battle Square"),
    (4, "Shinewood"),
    (5, "Heaven's Hell"),
    (6, "Firmament"),
    (7, "Purgatory"),
];

const CUSTOMIZE_KIND: &[(i64, &str)] = &[(1, "Coiffure"), (2, "Visage")];
const CUSTOMIZE_CASE: &[(i64, &str)] = &[(0, "Création et coiffeur"), (1, "Coiffeur seulement"), (2, "Création seulement")];

fn stat(name: &'static str, label: &'static str) -> Field {
    f(name, label).int().server()
}

fn data_sections() -> Vec<Section> {
    let mut elements = Vec::new();
    for e in ELEMENTS {
        elements.push(f(leak(format!("{e} Min_Damage")), leak(format!("{e} : dégâts min.")), ).int().server());
        elements.push(f(leak(format!("{e} Max_Damage")), leak(format!("{e} : dégâts max."))).int().server());
        elements.push(f(leak(format!("{e} Res.")), leak(format!("{e} : résistance (%)"))).int().server());
    }
    vec![
        section(
            "Combat",
            vec![
                f("LVL", "Niveau").int().server().help("Sert aussi de base à la mise à l'échelle des invocations."),
                stat("MAX_HP", "PV max."),
                stat("MAX_MP", "PM max."),
                stat("MAX_SP", "PE max."),
                stat("Min_Damage", "Dégâts min."),
                stat("Max_Damage", "Dégâts max."),
                stat("AR", "Précision"),
                stat("AC", "Défense physique"),
                stat("DR", "Esquive"),
                stat("Physical_Resistance", "Résistance physique (%)"),
                stat("Skill_Block", "Blocage des compétences (%)"),
                stat("ATK Speed", "Vitesse d'attaque").help("Plus la valeur est grande, plus l'attaque est rapide."),
                stat("Range", "Portée"),
                stat("Movement Walk", "Vitesse (marche)"),
                stat("Movement Run", "Vitesse (course)"),
                stat("Race", "Race").help("1 humain, 2 orc, 3 elfe de lune, 4 Dragon Scion."),
                stat("Class", "Classe").help("1 combattant, 2 tireur, 3 scion, 4 mage."),
                f("Type", "Type").flags(MONSTER_TYPE_BITS, "aucun").server(),
            ],
        ),
        section(
            "Récompenses et butin",
            vec![
                stat("Exp", "Expérience donnée"),
                stat("Geld_Min", "Or min."),
                stat("Geld_Max", "Or max."),
                stat("DropRank", "Rang de butin").help("Ligne de drop_rank_rate : rang des objets tirés dans les groupes de butin."),
                stat("DropMeditation", "Méditation").help("Multiplie le taux des groupes de butin « méditation » (1 par défaut)."),
            ],
        ),
        section(
            "Héroïque, siège, familier",
            vec![
                stat("Heroic_Min_Damage", "Dégâts héroïques min."),
                stat("Heroic_Max_Damage", "Dégâts héroïques max."),
                stat("Heroic_Defense_Point", "Défense héroïque"),
                f("SiegeWarCharType", "Objet de siège").enumeration(SIEGE_CHAR_TYPE).server(),
                stat("PetType", "Familier").help("1 : donne un sac de familier."),
            ],
        ),
        section(
            "Personnage de base (création)",
            vec![
                f("Weapon", "Arme de départ").server().help("TID, ou « TID;quantité »."),
                f("Item1", "Objet de départ 1").server().help("TID, ou « TID;quantité »."),
                f("Item2", "Objet de départ 2").server(),
                f("Item3", "Objet de départ 3").server(),
                f("Item4", "Objet de départ 4").server(),
                stat("STR", "Force"),
                stat("DEX", "Dextérité"),
                stat("INT", "Intelligence"),
                stat("CON", "Constitution"),
                stat("Wis", "Sagesse"),
                stat("Gender", "Sexe"),
            ],
        ),
        section("Éléments", elements),
    ]
}

fn creature() -> Entity {
    Entity {
        route: "creature",
        label: "Créatures",
        singular: "Créature",
        group: "Créatures et PNJ",
        db: "world",
        table: "character_template",
        key_col: "tid",
        key_label: "TID",
        key_text: false,
        reference: Some(RefKind::Creature),
        help: "Modèles de personnages : monstres, PNJ et personnages de base. Le TID doit exister dans les fichiers du \
               client (modèle 3D, animations) : un TID nouveau ne s'affiche pas sans eux. Le serveur charge ces tables \
               au démarrage : le redémarrer pour voir les changements.",
        create: Create::NextId("SELECT COALESCE(MAX(tid), 0) + 1 AS n FROM character_template"),
        search: Search {
            from: "character_template ct LEFT JOIN character_data_table d ON d.tid = ct.tid",
            columns: vec![
                col("ct.tid", "tid", "TID"),
                col(CREATURE_NAME, "name", "Nom"),
                col("ct.name", "model", "Nom du modèle"),
                col("COALESCE(JSON_VALUE(d.data, '$.LVL'), ct.level)", "level", "Niveau"),
                col_enum("ct.race", "race", "Race", RACE),
                col("(SELECT COUNT(*) FROM monster_spawn s WHERE s.tid = ct.tid)", "spawns", "Apparitions"),
            ],
            filters: vec![
                eq("tid", "TID", "ct.tid"),
                like("name", "Nom", CREATURE_NAME),
                eq_enum("race", "Race", "ct.race", RACE),
            ],
            order: "ct.tid",
            group_by: None,
        },
        tabs: vec![
            single(
                "template",
                "Modèle",
                "charactertemplate.ini : caractéristiques de base. Les personnages joueurs partent de ces valeurs, corrigées par la croissance.",
                SingleTab {
                    table: "character_template",
                    key: |id| by("tid", id),
                    main: true,
                    defaults: |_| row(&[("name", json!("Nouvelle créature")), ("character_type", json!(196608)), ("raw", json!({}))]),
                    on_copy: None,
                    fields: vec![
                        f("tid", "TID").int().ro().server(),
                        f("name", "Nom du modèle").server().help("Nom interne (souvent coréen) ; le nom affiché vient des caractéristiques serveur ou des traductions."),
                        f("character_type", "Type").int().help("Type du client : 196608 pour les monstres et PNJ, 0 pour les personnages de base."),
                        f("race", "Race").enumeration(RACE).server(),
                        f("gender", "Sexe").enumeration(GENDER).server(),
                        f("class", "Classe").enumeration(CLASS).server(),
                        stat("level", "Niveau"),
                        f("con", "Constitution").float().server().group("Caractéristiques"),
                        f("str", "Force").float().server().group("Caractéristiques"),
                        f("intel", "Intelligence").float().server().group("Caractéristiques"),
                        f("dex", "Dextérité").float().server().group("Caractéristiques"),
                        f("cha", "Charisme").float().group("Caractéristiques"),
                        f("luk", "Chance").float().server().group("Caractéristiques"),
                        f("wis", "Sagesse").float().server().group("Caractéristiques"),
                        stat("hp", "PV").group("Points"),
                        stat("mp", "PM").group("Points"),
                        stat("sp", "PE").group("Points"),
                        stat("movement", "Vitesse (marche)").group("Déplacement et combat"),
                        stat("movement_fast", "Vitesse (course)").group("Déplacement et combat"),
                        stat("attack_point", "Points d'attaque").group("Déplacement et combat"),
                        stat("magic_attack", "Attaque magique").group("Déplacement et combat"),
                        stat("ar", "Précision (AR)").group("Déplacement et combat"),
                        stat("dr", "Esquive (DR)").group("Déplacement et combat"),
                        stat("min_damage", "Dégâts min.").group("Déplacement et combat"),
                        stat("max_damage", "Dégâts max.").group("Déplacement et combat"),
                        stat("attack_range", "Portée d'attaque").group("Déplacement et combat"),
                        stat("attack_speed", "Vitesse d'attaque").group("Déplacement et combat"),
                        stat("base_exp", "Expérience de base").group("Déplacement et combat"),
                        f("sight", "Vue").int().group("Déplacement et combat"),
                        f("raw", "Clés de charactertemplate.ini").json(vec![section(
                            "Lue par le serveur",
                            vec![stat("HitRange", "Rayon du corps").help("Ajouté à la portée de l'attaquant (100 par défaut).")],
                        )]),
                    ],
                },
            ),
            single(
                "data",
                "Caractéristiques serveur",
                "CharacterDataTable.txt : les valeurs qu'utilise le serveur pour les monstres (PV, dégâts, expérience, or, butin) et l'équipement de départ des personnages de base.",
                SingleTab {
                    table: "character_data_table",
                    key: |id| by("tid", id),
                    main: false,
                    defaults: |id| row(&[("name", json!("Nouvelle créature")), ("data", json!({ "TID": id.to_string() }))]),
                    on_copy: Some(|mut r, id| {
                        if let Some(Value::Object(d)) = r.get_mut("data") {
                            d.insert("TID".into(), Value::String(id.to_string()));
                        }
                        r
                    }),
                    fields: vec![
                        f("tid", "TID").int().ro(),
                        f("name", "Nom").server().help("Nom affiché du monstre (repli des traductions monster_name) ; les personnages de base sont reconnus à leur nom (Knight, Archer…)."),
                        f("data", "Colonnes de CharacterDataTable.txt").json(data_sections()),
                    ],
                },
            ),
            multi(
                "spawns",
                "Apparitions",
                "Points d'apparition du monstre. Une ligne est visée par sa clé stable (point#rang), la même d'un import à l'autre ; les lignes ajoutées reçoivent une clé #af….",
                MultiTab {
                    table: "monster_spawn",
                    parent: |id| by("tid", id),
                    order: "id",
                    row_key: vec!["stable_key"],
                    auto_id: Some("id"),
                    auto_increment: None,
                    copy: false,
                    columns: vec![
                        f("stable_key", "Clé stable").ro(),
                        f("name", "Point").help("Point de spawninstancetable.txt."),
                        f("display_name", "Nom public").server(),
                        stat("level", "Niv."),
                        stat("count", "Nombre").help("Monstres sur ce point, multiplié par World.MonsterDensity ; 0 : point ignoré."),
                        stat("respawn_sec", "Réapp. (s)"),
                        stat("respawn_max_sec", "Réapp. max (s)").help("Délai tiré entre respawn_sec et cette valeur à chaque mort ; 0 : délai fixe."),
                        f("ai_tid", "IA").reference(RefKind::MonsterAi).server(),
                        stat("siege_war", "Siège").help("1 objets, 2 gardes : cachés hors guerre de siège ; 0 : hors siège."),
                        f("pos_x", "X").float().server(),
                        f("pos_y", "Y").float().server().help("0 : posé au sol."),
                        f("pos_z", "Z").float().server(),
                        f("degree_x", "Orient.").float().server(),
                        f("radius", "Rayon").float().server().help("Dispersion autour du point."),
                    ],
                    new_row: |id, _, last| match last {
                        Some(l) => l.clone(),
                        None => row(&[
                            ("stable_key", json!("")),
                            ("group_name", json!("archforge")),
                            ("name", json!(format!("af{id}"))),
                            ("tid", id.clone()),
                            ("display_name", json!("")),
                            ("level", json!(1)),
                            ("count", json!(1)),
                            ("max_count", json!(0)),
                            ("respawn_sec", json!(30)),
                            ("respawn_max_sec", json!(0)),
                            ("rate", json!(10000)),
                            ("ai_tid", json!(0)),
                            ("siege_war", json!(0)),
                            ("is_boss", json!(0)),
                            ("pos_x", json!(0)),
                            ("pos_y", json!(0)),
                            ("pos_z", json!(0)),
                            ("degree_x", json!(0)),
                            ("degree_y", json!(0)),
                            ("radius", json!(500)),
                        ]),
                    },
                    fresh: Some(|mut r| {
                        let point = r.get("name").and_then(|v| v.as_str()).unwrap_or("af").to_string();
                        r.insert("stable_key".into(), Value::String(forge_key(&point, 48)));
                        r
                    }),
                },
            ),
            multi(
                "loot",
                "Butin",
                "Objets propres au monstre (itemdroptable.txt), en plus des groupes de butin d'item_data. Taux sur 100 000 : 100 000 = toujours.",
                MultiTab {
                    table: "monster_drop",
                    parent: |id| by("monster_tid", id),
                    order: "id",
                    row_key: vec!["stable_key"],
                    auto_id: Some("id"),
                    auto_increment: None,
                    copy: true,
                    columns: vec![
                        f("stable_key", "Clé stable").ro(),
                        f("item_tid", "Objet").reference(RefKind::Item).server(),
                        stat("min_count", "Min."),
                        stat("max_count", "Max."),
                        stat("rate", "Taux / 100 000"),
                    ],
                    new_row: |id, _, _| row(&[("stable_key", json!("")), ("monster_tid", id.clone()), ("item_tid", json!(0)), ("min_count", json!(1)), ("max_count", json!(1)), ("rate", json!(1000))]),
                    fresh: Some(|mut r| {
                        let k = format!("{}:{}", crate::editor::display(r.get("monster_tid").unwrap_or(&Value::Null)), crate::editor::display(r.get("item_tid").unwrap_or(&Value::Null)));
                        r.insert("stable_key".into(), Value::String(forge_key(&k, 40)));
                        r
                    }),
                },
            ),
            single(
                "gather",
                "Récolte",
                "product_gather : le cadavre du monstre se récolte (compétence de collecte) ; les objets viennent de l'ensemble de récolte.",
                SingleTab {
                    table: "product_gather",
                    key: |id| by("monster_tid", id),
                    main: false,
                    defaults: |_| row(&[("classes", json!(30)), ("skill_tid", json!(663)), ("rate", json!(100)), ("item_set", json!(0))]),
                    on_copy: None,
                    fields: vec![
                        f("monster_tid", "Monstre").int().ro(),
                        f("classes", "Classes admises").flags(CLASS_BITS, "aucune").server(),
                        f("skill_tid", "Compétence").reference(RefKind::Skill).server(),
                        stat("rate", "Chance de réussite"),
                        f("item_set", "Ensemble d'objets").reference(RefKind::ItemSet).server(),
                    ],
                },
            ),
            multi(
                "growth",
                "Croissance",
                "GrowUpFactor.txt : valeurs absolues par niveau pour les modèles jouables ; seules les colonnes non nulles remplacent celles du modèle, et un trou dans les niveaux arrête le bloc.",
                MultiTab {
                    table: "growup_factor",
                    parent: |id| by("template_id", id),
                    order: "level",
                    row_key: vec!["template_id", "level"],
                    auto_id: None,
                    auto_increment: None,
                    copy: true,
                    columns: ["level", "str", "dex", "con", "wis", "intel", "cha", "hp", "mp", "sp", "ac", "ar", "dr", "min_damage", "max_damage"]
                        .into_iter()
                        .map(|c| stat(c, c))
                        .collect(),
                    new_row: |_, _, last| {
                        let mut r = last.cloned().unwrap_or_default();
                        let level = last.and_then(|l| l.get("level")).and_then(|v| v.as_i64()).unwrap_or(0) + 1;
                        r.insert("level".into(), json!(level));
                        r
                    },
                    fresh: None,
                },
            ),
            multi(
                "customize",
                "Apparence",
                "charactercustomizelist.txt : coiffures et visages proposés à la création et chez le coiffeur, pour un modèle jouable.",
                MultiTab {
                    table: "character_customize",
                    parent: |id| by("character_tid", id),
                    order: "row_order",
                    row_key: vec!["character_tid", "kind", "number"],
                    auto_id: Some("row_order"),
                    auto_increment: None,
                    copy: true,
                    columns: vec![
                        f("row_order", "Ordre").int().ro().help("Ordre du fichier (calcul de prix du client) ; donné à l'enregistrement."),
                        f("kind", "Sorte").enumeration(CUSTOMIZE_KIND).server(),
                        stat("number", "Numéro").help("Numéro envoyé au client."),
                        f("name", "Nom").server(),
                        f("case_kind", "Où").enumeration(CUSTOMIZE_CASE).server(),
                        stat("use_level", "Niveau"),
                        stat("price", "Prix"),
                        f("price_item_tid", "Payé en objet").reference(RefKind::Item).server(),
                    ],
                    new_row: |id, rows, _| {
                        let number = rows.iter().filter_map(|r| r.get("number").and_then(|v| v.as_i64())).max().map_or(0, |n| n + 1);
                        row(&[("row_order", Value::Null), ("kind", json!(1)), ("number", json!(number)), ("name", json!("")), ("case_kind", json!(0)), ("character_tid", id.clone()), ("use_level", json!(1)), ("price", json!(0)), ("price_item_tid", json!(0))])
                    },
                    fresh: None,
                },
            ),
            single(
                "boss",
                "Fenêtre des boss",
                "UIBoss.xml : boss affichés dans la fenêtre des boss d'Archonia (32 au plus), avec leur état et leur prochaine apparition.",
                SingleTab {
                    table: "boss_window",
                    key: |id| by("tid", id),
                    main: false,
                    defaults: |_| row(&[("row_order", json!(31)), ("category", json!(0)), ("name", json!(""))]),
                    on_copy: None,
                    fields: vec![
                        f("tid", "TID").int().ro(),
                        f("row_order", "Ordre").int().server(),
                        f("category", "Catégorie").enumeration(BOSS_CATEGORY).server(),
                        f("name", "Nom").server(),
                    ],
                },
            ),
            localized_tab!("monster_name"),
        ],
    }
}

/// Fonctions d'un PNJ (npc.ini, `world/npcs.rs`).
pub const NPC_FUNCTIONS: &[(i64, &str)] = &[
    (9, "Boutique"),
    (17, "Réparation"),
    (20, "Banque"),
    (21, "Dialogue"),
    (22, "Conversion"),
    (23, "Guilde"),
    (24, "Production"),
    (25, "Maître des compétences"),
    (27, "Quêtes"),
    (28, "Maison des ventes"),
    (29, "Coiffeur"),
    (32, "Avis de recherche"),
    (33, "Commandant de siège"),
    (34, "Taxes"),
    (35, "Entrepôt de guilde"),
    (36, "ArchLord"),
];

fn npc() -> Entity {
    Entity {
        route: "npc",
        label: "PNJ",
        singular: "PNJ",
        group: "Créatures et PNJ",
        db: "world",
        table: "npc_spawn",
        key_col: "id",
        key_label: "Identifiant",
        key_text: false,
        reference: Some(RefKind::Npc),
        help: "PNJ placés dans le monde : modèle, position et fonctions (dialogue, boutique, quêtes…) telles que dans npc.ini.",
        // Les sections de npc.ini vont jusqu'à quelques milliers : un PNJ créé ici prend un numéro à part.
        create: Create::NextId("SELECT GREATEST(COALESCE(MAX(id), 0) + 1, 100000) AS n FROM npc_spawn"),
        search: Search {
            from: "npc_spawn",
            columns: vec![
                col("id", "id", "ID"),
                col("name", "name", "Nom"),
                col("tid", "tid", "Modèle (TID)"),
                col("ROUND(pos_x)", "x", "X"),
                col("ROUND(pos_z)", "z", "Z"),
            ],
            filters: vec![eq("id", "ID", "id"), like("name", "Nom", "name"), eq("tid", "Modèle (TID)", "tid")],
            order: "id",
            group_by: None,
        },
        tabs: vec![
            single(
                "npc",
                "PNJ",
                "",
                SingleTab {
                    table: "npc_spawn",
                    key: |id| by("id", id),
                    main: true,
                    defaults: |id| {
                        let event = 1_073_741_824 + id.as_i64().unwrap_or(0);
                        row(&[
                            ("tid", json!(584)),
                            ("name", json!("<Merchant> Nouveau")),
                            ("pos_x", json!(0)),
                            ("pos_y", json!(0)),
                            ("pos_z", json!(0)),
                            ("degree_x", json!(0)),
                            ("degree_y", json!(0)),
                            (
                                "raw",
                                json!([
                                    ["ModuleData1", "AgpmCharacter"],
                                    ["Name", "<Merchant> Nouveau"],
                                    ["TID", "584"],
                                    ["ModuleData2", "ApmEventManager"],
                                    ["Function", "9"],
                                    ["EventID", event.to_string()],
                                    ["CondStart", "0"],
                                    ["CondEnd", "0"],
                                    ["NPCTradeStart", "0"],
                                    ["Template", "104"],
                                    ["NPCTradeEnd", "0"],
                                    ["EnumEnd", "0"]
                                ]),
                            ),
                        ])
                    },
                    on_copy: None,
                    fields: vec![
                        f("id", "ID").int().ro().help("Section [N] de npc.ini ; le CID en jeu vaut 0x40000000 + id."),
                        f("tid", "Modèle").reference(RefKind::Creature).server().help("Modèle de personnage (apparence)."),
                        f("name", "Nom").server().help("« <Titre> Nom » : le titre entre chevrons est traduit par la table des titres ou les traductions npc_name."),
                        f("pos_x", "X").float().server().group("Position"),
                        f("pos_y", "Y (hauteur)").float().server().group("Position"),
                        f("pos_z", "Z").float().server().group("Position"),
                        f("degree_x", "Angle X").float().server().group("Position"),
                        f("degree_y", "Orientation").float().server().group("Position").help("Rotation autour de l'axe vertical, en degrés."),
                        f("raw", "Fonctions (npc.ini)").pairs().server().help(
                            "Chaque « Function » ouvre un événement ; ses paramètres suivent. Lus par le serveur : Template (21 dialogue, 9 boutique), \
                             QuestGroupID (27 quêtes), ProductCategory (24 production), SkillRaceType et SkillClassType (25 maître des compétences). \
                             L'ordre des paires compte.",
                        ),
                    ],
                },
            ),
            localized_tab!("npc_name"),
        ],
    }
}

fn monster_ai() -> Entity {
    let skill = |n: u8| {
        [
            f(leak(format!("Skill{n}")), leak(format!("Compétence {n}"))).server().help("« TID;niveau »."),
            f(leak(format!("SkillCondition{n}")), leak(format!("Condition {n}"))).server().help("« HP;100%;이하 » : au plus (이하) ou au moins (이상) tant de PV."),
            f(leak(format!("SkillCount{n}")), leak(format!("Nombre {n}"))).server().help("« 무한 » : sans limite ; « 횟수;n » : n fois par vie."),
        ]
    };
    let skills: Vec<Field> = (1..=5).flat_map(skill).collect();
    Entity {
        route: "monster-ai",
        label: "IA des monstres",
        singular: "IA de monstre",
        group: "Créatures et PNJ",
        db: "world",
        table: "monster_ai",
        key_col: "tid",
        key_label: "TID",
        key_text: false,
        reference: Some(RefKind::MonsterAi),
        help: "aidatatable.txt : compétences, préférences de cible et suivants d'un monstre. Une apparition désigne son IA par ai_tid.",
        create: Create::NextId("SELECT COALESCE(MAX(tid), 0) + 1 AS n FROM monster_ai"),
        search: Search {
            from: "monster_ai",
            columns: vec![
                col("tid", "tid", "TID"),
                col("name", "name", "Nom"),
                col("JSON_VALUE(data, '$.Skill1')", "skill1", "Compétence 1"),
                col("(SELECT COUNT(*) FROM monster_spawn s WHERE s.ai_tid = monster_ai.tid)", "spawns", "Apparitions"),
            ],
            filters: vec![eq("tid", "TID", "tid"), like("name", "Nom", "name")],
            order: "tid",
            group_by: None,
        },
        tabs: vec![
            single(
                "ai",
                "IA",
                "",
                SingleTab {
                    table: "monster_ai",
                    key: |id| by("tid", id),
                    main: true,
                    defaults: |_| row(&[("name", json!("Nouvelle IA")), ("data", json!({"UseNormalAttack": "1"}))]),
                    on_copy: None,
                    fields: vec![
                        f("tid", "TID").int().ro(),
                        f("name", "Nom"),
                        f("data", "Colonnes d'aidatatable.txt").json(vec![
                            section("Compétences", skills),
                            section(
                                "Attaque préventive",
                                vec![
                                    stat("PreemptiveType", "Préventive").help("1 : attaque sans attendre d'être frappé."),
                                    f("PreemptiveCondition1", "Condition 1").server().help("« 타겟;LV;n;이상|이하;금지 » : cible de niveau au moins / au plus n interdite."),
                                    f("PreemptiveCondition2", "Condition 2").server(),
                                    f("PreemptiveCondition3", "Condition 3").server(),
                                    stat("PreemptiveRange", "Portée").help("En mètres (× 100 en unités du jeu)."),
                                ],
                            ),
                            section(
                                "Préférences de cible",
                                vec![
                                    f("HateRace", "Race détestée").server().help("인간 humain, 오크 orc, 문엘프 elfe de lune."),
                                    stat("HateRacePercent", "Haine de race (%)"),
                                    f("HateClass", "Classe détestée").server().help("전사 combattant, 궁수 tireur, 법사 mage."),
                                    stat("HateClassPercent", "Haine de classe (%)"),
                                    stat("UseNormalAttack", "Coup normal").help("0 : pas d'attaque normale."),
                                ],
                            ),
                            section(
                                "Invocations et suivants",
                                vec![
                                    f("SummonTID", "Invocations").server().help("TID séparés par « ; »."),
                                    stat("SummonMaxCount", "Invocations max."),
                                    f("FollowerTID", "Suivants").server().help("TID séparés par « ; »."),
                                    stat("FollowerNUM", "Nombre de suivants"),
                                    stat("Max_CumulativeFollower", "Suivants cumulés max."),
                                    stat("Min_UpkeepFollower", "Suivants gardés min."),
                                    f("PinchMonster", "Renfort en danger").server(),
                                ],
                            ),
                        ]),
                    ],
                },
            ),
            list(
                "spawns",
                "Apparitions",
                "Apparitions de monstres qui utilisent cette IA.",
                ListTab {
                    query: |id| {
                        (
                            "SELECT tid, MIN(display_name) AS display_name, COUNT(*) AS points FROM monster_spawn WHERE ai_tid = ? GROUP BY tid ORDER BY tid".into(),
                            vec![id.clone()],
                        )
                    },
                    columns: vec![("tid", "Monstre", Some(RefKind::Creature)), ("display_name", "Nom public", None), ("points", "Points", None)],
                    empty: "Aucune apparition n'utilise cette IA.",
                },
            ),
        ],
    }
}

pub fn entities() -> Vec<Entity> {
    vec![creature(), npc(), monster_ai()]
}
