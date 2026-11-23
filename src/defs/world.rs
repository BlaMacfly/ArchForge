//! Monde : téléportation, régions et résurrection, Shrines, guerres de siège, crânes de la Battle Square.

use serde_json::{json, Value};

use super::{forge_key, CLASS, RACE, RACE_BITS, SAFETY};
use crate::localized_tab;
use crate::model::*;

fn s(name: &'static str, label: &'static str) -> Field {
    f(name, label).int().server()
}

const TARGET_TYPE: &[(i64, &str)] = &[(1, "Position fixe"), (2, "Objet (portail)")];
const REGION_DISABLED: &[(i64, &str)] = &[(1, "Rappel de groupe interdit"), (2, "/go interdit"), (4, "Groupes interdits"), (8, "Entrée en jeu interdite")];
const REGION_TYPE_BITS: &[(i64, &str)] = &[(0x10, "Montures permises"), (0x100, "Résurrection permise")];
const DAYS: &[(i64, &str)] = &[(1, "Lundi"), (2, "Mardi"), (4, "Mercredi"), (8, "Jeudi"), (16, "Vendredi"), (32, "Samedi"), (64, "Dimanche")];
const SHRINE_REWARD: &[(&str, &str)] = &[("Carving", "Carving (au graveur)"), ("WinRace", "WinRace (race victorieuse présente)"), ("Common", "Common (tout joueur présent)")];
const BINDING_TYPE: &[(i64, &str)] = &[(3, "Assaillants (abords)"), (4, "Défense intérieure"), (5, "ArchLord"), (6, "Assaillants de l'ArchLord"), (7, "Défense extérieure")];
const SIEGE_OBJECT: &[(i64, &str)] = &[(9, "Camp de résurrection des assiégeants"), (10, "Catapulte")];

fn teleport() -> Entity {
    Entity {
        route: "teleport",
        label: "Téléportation",
        singular: "Point de téléportation",
        group: "Monde",
        db: "world",
        table: "teleport_point",
        key_col: "name",
        key_label: "Nom du point",
        key_text: true,
        reference: Some(RefKind::TeleportPoint),
        help: "TeleportPoint.ini : un portail mène des points de ses groupes de destination vers les points de ces groupes. Le prix de chaque trajet vient de TeleportFee.txt.",
        create: Create::Manual,
        search: Search {
            from: "teleport_point",
            columns: vec![
                col("point_id", "point_id", "N°"),
                col("name", "name", "Nom"),
                col_enum("target_type", "target_type", "Arrivée", TARGET_TYPE),
                col("point_groups", "point_groups", "Groupes"),
                col("target_groups", "target_groups", "Destinations"),
                col("(SELECT COUNT(*) FROM teleport_object o WHERE o.point = teleport_point.name)", "objects", "Portails"),
            ],
            filters: vec![like("name", "Nom", "name"), like("group", "Groupe", "point_groups")],
            order: "point_id, name",
            group_by: None,
        },
        tabs: vec![
            single(
                "point",
                "Point",
                "",
                SingleTab {
                    table: "teleport_point",
                    key: |id| by("name", id),
                    main: true,
                    defaults: |_| {
                        row(&[
                            ("point_id", json!(0)),
                            ("target_type", json!(1)),
                            ("pos_x", json!(0)),
                            ("pos_y", json!(0)),
                            ("pos_z", json!(0)),
                            ("radius_min", json!(0)),
                            ("radius_max", json!(300)),
                            ("region_type", json!(0)),
                            ("special_type", json!(0)),
                            ("use_type", json!(0)),
                            ("point_groups", json!([])),
                            ("target_groups", json!([])),
                        ])
                    },
                    on_copy: None,
                    fields: vec![
                        f("name", "Nom").ro().server().help("Nom du point (sensible à la casse), clé des portails et des prix."),
                        s("point_id", "Numéro").help("Rang dans TeleportPoint.ini, à partir de 1, comme au chargement du client : il désigne les points désactivés envoyés à l'accord d'un portail."),
                        f("target_type", "Type d'arrivée").enumeration(TARGET_TYPE).server(),
                        f("pos_x", "X").float().server().group("Arrivée"),
                        f("pos_y", "Y").float().server().group("Arrivée"),
                        f("pos_z", "Z").float().server().group("Arrivée"),
                        f("radius_min", "Rayon min.").float().server().group("Arrivée").help("Distance minimale de l'arrivée au point."),
                        f("radius_max", "Rayon max.").float().server().group("Arrivée"),
                        f("region_type", "Région").enumeration(&[(0, "Normale"), (1, "Combat entre joueurs")]).server(),
                        s("special_type", "Type spécial").help("1 : retour seul."),
                        f("use_type", "Races admises").flags(RACE_BITS, "toutes").server(),
                        f("point_groups", "Groupes (arrivée)").text_list().server().help("Groupes dont ce point est une arrivée."),
                        f("target_groups", "Destinations").text_list().server().help("Groupes vers lesquels ce point mène."),
                    ],
                },
            ),
            multi(
                "fees",
                "Prix des trajets",
                "Prix payé pour aller de ce point vers chaque destination.",
                MultiTab {
                    table: "teleport_fee",
                    parent: |id| by("source", id),
                    order: "destination",
                    row_key: vec!["source", "destination"],
                    auto_id: None,
                    auto_increment: None,
                    copy: true,
                    columns: vec![f("destination", "Destination").reference(RefKind::TeleportPoint).server(), s("fee", "Prix")],
                    new_row: |id, _, _| row(&[("source", id.clone()), ("destination", json!("")), ("fee", json!(0))]),
                    fresh: None,
                },
            ),
            list(
                "objects",
                "Portails",
                "Objets du monde (obj*.ini, événement 8) qui portent ce point. Ils viennent des fichiers du client.",
                ListTab {
                    query: |id| ("SELECT object_id, pos_x, pos_y, pos_z FROM teleport_object WHERE point = ? ORDER BY object_id".into(), vec![id.clone()]),
                    columns: vec![("object_id", "Objet", None), ("pos_x", "X", None), ("pos_y", "Y", None), ("pos_z", "Z", None)],
                    empty: "Aucun portail ne porte ce point.",
                },
            ),
        ],
    }
}

fn region() -> Entity {
    Entity {
        route: "region",
        label: "Régions",
        singular: "Région",
        group: "Monde",
        db: "world",
        table: "region_template",
        key_col: "id",
        key_label: "Numéro",
        key_text: false,
        reference: Some(RefKind::Region),
        help: "RegionTemplate.ini : le numéro de région vient de la carte du client (map_segment) ; la sûreté décide du combat entre joueurs.",
        create: Create::No,
        search: Search {
            from: "region_template",
            columns: vec![
                col("id", "id", "ID"),
                col("name", "name", "Nom"),
                col("parent_id", "parent_id", "Parente"),
                col_enum("safety", "safety", "Sûreté", SAFETY),
                col("level_min", "level_min", "Niv. min."),
                col("(SELECT COUNT(*) FROM resurrection_point r WHERE r.region_id = region_template.id)", "res", "Résurrections"),
            ],
            filters: vec![eq("id", "ID", "id"), like("name", "Nom", "name"), eq_enum("safety", "Sûreté", "safety", SAFETY)],
            order: "id",
            group_by: None,
        },
        tabs: vec![
            single(
                "region",
                "Région",
                "",
                SingleTab {
                    table: "region_template",
                    key: |id| by("id", id),
                    main: true,
                    defaults: |_| row(&[]),
                    on_copy: None,
                    fields: vec![
                        f("id", "ID").int().ro().server(),
                        f("name", "Nom").server().help("Nom d'origine (coréen) ; traduit par region_name et par le glossaire des régions."),
                        f("parent_id", "Région parente").reference(RefKind::Region).server().help("-1 : aucune."),
                        f("type_bits", "Champ Type").flags(REGION_TYPE_BITS, "aucun").server().help("Bits 0-1 terrain, 2-3 sûreté ; 0x10 montures permises ; 0x100 résurrection permise."),
                        f("safety", "Sûreté").enumeration(SAFETY).server(),
                        s("level_min", "Niveau min. d'entrée").group("Règles").help("0 : aucun. Étages de la Battle Square seulement."),
                        s("level_limit", "Niveau max. conseillé").group("Règles"),
                        f("disabled", "Interdits").flags(REGION_DISABLED, "aucun").server().group("Règles"),
                        s("item_section", "Section d'objets exclue").group("Règles").help("Les objets de ce Item_section_num n'y comptent pas (1 : Heaven's Hell) ; 0 : aucune."),
                        f("resurrection_x", "Résurrection X").float().server().group("Règles").help("Point de résurrection propre à la région ; 0 : aucun."),
                        f("resurrection_z", "Résurrection Z").float().server().group("Règles"),
                    ],
                },
            ),
            multi(
                "resurrection",
                "Résurrection",
                "Points où renaissent les joueurs morts dans la région. Une ligne sans clé stable (import d'avant la migration) ne peut pas être visée : relancer `archcore import`.",
                MultiTab {
                    table: "resurrection_point",
                    parent: |id| by("region_id", id),
                    order: "id",
                    row_key: vec!["stable_key"],
                    auto_id: Some("id"),
                    auto_increment: None,
                    copy: false,
                    columns: vec![
                        f("stable_key", "Clé stable").ro(),
                        f("name", "Nom"),
                        f("pos_x", "X").float().server(),
                        f("pos_y", "Y").float().server(),
                        f("pos_z", "Z").float().server(),
                        f("race", "Race").enumeration(RACE).server(),
                        f("class", "Classe").enumeration(CLASS).server(),
                    ],
                    new_row: |id, _, _| row(&[("stable_key", json!("")), ("region_id", id.clone()), ("name", json!("")), ("pos_x", json!(0)), ("pos_y", json!(0)), ("pos_z", json!(0)), ("race", json!(0)), ("class", json!(0))]),
                    fresh: Some(|mut r| {
                        let k = format!("archforge#{}", crate::editor::display(r.get("region_id").unwrap_or(&Value::Null)));
                        r.insert("stable_key".into(), Value::String(forge_key(&k, 64)));
                        r
                    }),
                },
            ),
            list(
                "glossary",
                "Glossaire",
                "RegionGlossary.txt : nom de la région dans chaque langue du client ; le serveur l'envoie pour le taux de taxe. Rempli par l'import.",
                ListTab {
                    query: |id| ("SELECT g.lang, g.name FROM region_glossary g JOIN region_template r ON r.name = g.korean_name WHERE r.id = ? ORDER BY g.lang".into(), vec![id.clone()]),
                    columns: vec![("lang", "Langue", Some(RefKind::Language)), ("name", "Nom", None)],
                    empty: "Aucune entrée de glossaire pour cette région.",
                },
            ),
            localized_tab!("region_name"),
        ],
    }
}

fn shrine() -> Entity {
    Entity {
        route: "shrine",
        label: "Shrines",
        singular: "Shrine",
        group: "Monde",
        db: "world",
        table: "shrine",
        key_col: "id",
        key_label: "Shrine",
        key_text: false,
        reference: None,
        help: "Combats de Shrine de l'épisode 8. Ces tables sont propres à ArchCore : elles sont remplies par la couche de contenu \
               (content/202610281700_shrines.sql) — y ajouter les changements pour qu'ils survivent à `archcore import`.",
        create: Create::NextId("SELECT COALESCE(MAX(id), 0) + 1 AS n FROM shrine"),
        search: Search {
            from: "shrine",
            columns: vec![
                col("id", "id", "ID"),
                col("name", "name", "Nom"),
                col("region_id", "region_id", "Région"),
                col("CONCAT(LPAD(open_hour, 2, '0'), ':', LPAD(open_minute, 2, '0'))", "open", "Ouverture"),
                col("duration_min", "duration_min", "Durée (min)"),
            ],
            filters: vec![],
            order: "id",
            group_by: None,
        },
        tabs: vec![single(
            "shrine",
            "Shrine",
            "Un joueur de la région centrale grave le sceau en carve_sec secondes, puis sa race doit le garder occupy_sec secondes.",
            SingleTab {
                table: "shrine",
                key: |id| by("id", id),
                main: true,
                defaults: |_| {
                    row(&[
                        ("name", json!("Nouveau Shrine")),
                        ("region_id", json!(236)),
                        ("days", json!(127)),
                        ("open_hour", json!(20)),
                        ("open_minute", json!(0)),
                        ("duration_min", json!(30)),
                        ("carve_sec", json!(60)),
                        ("occupy_sec", json!(300)),
                        ("level_min", json!(50)),
                        ("level_max", json!(0)),
                        ("max_per_race", json!(50)),
                    ])
                },
                on_copy: None,
                fields: vec![
                    f("id", "ID").int().ro().server().help("1 à 10."),
                    f("name", "Nom").server().help("29 caractères au plus."),
                    f("region_id", "Région centrale").reference(RefKind::Region).server().help("236 à 243 ; sinon le Shrine est ignoré."),
                    f("days", "Jours d'ouverture").flags(DAYS, "aucun").server(),
                    s("open_hour", "Heure").group("Horaire").help("Heure de World.TimeZone."),
                    s("open_minute", "Minute").group("Horaire"),
                    s("duration_min", "Durée (min)").group("Horaire"),
                    s("carve_sec", "Gravure (s)").group("Combat"),
                    s("occupy_sec", "Occupation (s)").group("Combat"),
                    s("level_min", "Niveau min.").group("Entrée"),
                    s("level_max", "Niveau max.").group("Entrée").help("0 : sans limite."),
                    s("max_per_race", "Joueurs par race").group("Entrée"),
                ],
            },
        )],
    }
}

fn shrine_reward() -> Entity {
    Entity {
        route: "shrine-reward",
        label: "Récompenses des Shrines",
        singular: "Récompenses",
        group: "Monde",
        db: "world",
        table: "shrine_reward",
        key_col: "category",
        key_label: "Catégorie",
        key_text: true,
        reference: None,
        help: "ShrineReward.xml : récompenses d'un combat gagné. Table propre à ArchCore, remplie par la couche de contenu.",
        create: Create::No,
        search: Search {
            from: "shrine_reward",
            columns: vec![col("category", "category", "Catégorie"), col("COUNT(*)", "n", "Lignes"), col("SUM(charisma)", "charisma", "Charisme")],
            filters: vec![],
            order: "category",
            group_by: Some("category"),
        },
        tabs: vec![multi(
            "rewards",
            "Récompenses",
            "Objet item_tid × item_count (0 : aucun) et points de charisme.",
            MultiTab {
                table: "shrine_reward",
                parent: |id| by("category", id),
                order: "item_tid",
                row_key: vec!["category", "item_tid"],
                auto_id: None,
                auto_increment: None,
                copy: false,
                columns: vec![
                    f("category", "Catégorie").text_enum(SHRINE_REWARD).ro(),
                    f("item_tid", "Objet").reference(RefKind::Item).server(),
                    s("item_count", "Quantité"),
                    s("charisma", "Charisme"),
                ],
                new_row: |id, _, _| row(&[("category", id.clone()), ("item_tid", json!(0)), ("item_count", json!(0)), ("charisma", json!(0))]),
                fresh: None,
            },
        )],
    }
}

fn castle() -> Entity {
    Entity {
        route: "castle",
        label: "Châteaux",
        singular: "Château",
        group: "Sièges",
        db: "world",
        table: "siege_war",
        key_col: "id",
        key_label: "Château",
        key_text: false,
        reference: Some(RefKind::Castle),
        help: "SiegeWar.txt : une ligne par château (0 Rensphere, 1 Inflame, 2 Arthian, 3 Traqua, 4 Drayger).",
        create: Create::No,
        search: Search {
            from: "siege_war",
            columns: vec![col("id", "id", "ID"), col("name", "name", "Nom"), col("castle_region", "castle_region", "Région"), col("begin_hour", "begin_hour", "Heure"), col("disabled", "disabled", "Désactivé")],
            filters: vec![],
            order: "id",
            group_by: None,
        },
        tabs: vec![
            single(
                "castle",
                "Château",
                "",
                SingleTab {
                    table: "siege_war",
                    key: |id| by("id", id),
                    main: true,
                    defaults: |_| row(&[]),
                    on_copy: None,
                    fields: vec![
                        f("id", "ID").int().ro().server(),
                        f("name", "Nom").server().help("Nom du château, clé des paquets."),
                        f("castle_region", "Région du château").reference(RefKind::Region).server(),
                        f("outskirts_region", "Région des abords").reference(RefKind::Region).server(),
                        s("playing_ms", "Durée du siège (ms)").group("Durées"),
                        s("eye_open_ms", "Yeux d'Archon (ms)").group("Durées").help("0 : pas d'Yeux d'Archon."),
                        s("stamping_ms", "Durée du carving (ms)").group("Durées"),
                        s("declare_hours", "Déclaration (h)").group("Calendrier"),
                        s("select_hours", "Candidature (h)").group("Calendrier"),
                        s("interval_days", "Écart entre sièges (jours)").group("Calendrier").help("1 au moins."),
                        s("begin_hour", "Heure de début").group("Calendrier").help("23 au plus."),
                        f("stamp_skill_tid", "Compétence du carving").reference(RefKind::Skill).server(),
                        s("stamp_skill_level", "Niveau de la compétence"),
                        f("disabled", "Désactivé").bool().server(),
                    ],
                },
            ),
            list(
                "bindings",
                "Résurrection",
                "Points de résurrection des sièges (obj*.ini du client) dans les régions du château et de ses abords.",
                ListTab {
                    query: |id| {
                        (
                            "SELECT b.region_id, b.binding_type, b.name, b.pos_x, b.pos_y, b.pos_z FROM siege_binding b JOIN siege_war w \
                             ON b.region_id IN (w.castle_region, w.outskirts_region) WHERE w.id = ? ORDER BY b.binding_type, b.id"
                                .into(),
                            vec![id.clone()],
                        )
                    },
                    columns: vec![("region_id", "Région", Some(RefKind::Region)), ("binding_type", "Type", None), ("name", "Nom", None), ("pos_x", "X", None), ("pos_y", "Y", None), ("pos_z", "Z", None)],
                    empty: "Aucun point de résurrection de siège.",
                },
            ),
        ],
    }
}

fn siege_object() -> Entity {
    Entity {
        route: "siege-object",
        label: "Objets de siège",
        singular: "Objet de siège",
        group: "Sièges",
        db: "world",
        table: "siege_war_object",
        key_col: "type",
        key_label: "Type",
        key_text: false,
        reference: None,
        help: "SiegeWarObject.txt : objets d'assaut réparables.",
        create: Create::No,
        search: Search {
            from: "siege_war_object",
            columns: vec![col_enum("type", "type", "Type", SIEGE_OBJECT), col("description", "description", "Description"), col("repair_cost", "repair_cost", "Coût")],
            filters: vec![],
            order: "type",
            group_by: None,
        },
        tabs: vec![single(
            "object",
            "Objet",
            "",
            SingleTab {
                table: "siege_war_object",
                key: |id| by("type", id),
                main: true,
                defaults: |_| row(&[]),
                on_copy: None,
                fields: vec![
                    f("type", "Type").enumeration(SIEGE_OBJECT).ro().server(),
                    f("description", "Description"),
                    s("repair_count", "Réparations"),
                    s("repair_cost", "Coût d'une réparation"),
                    s("repair_minutes", "Durée d'une réparation (min)"),
                    f("items", "Objets à charger").pair_list(Some(RefKind::Item), "Objet", "Nombre").server(),
                ],
            },
        )],
    }
}

fn siege_binding() -> Entity {
    Entity {
        route: "siege-binding",
        label: "Résurrection de siège",
        singular: "Point de résurrection de siège",
        group: "Sièges",
        db: "world",
        table: "siege_binding",
        key_col: "id",
        key_label: "ID",
        key_text: false,
        reference: None,
        help: "Événements « binding » des obj*.ini : où renaissent attaquants et défenseurs pendant un siège. L'identifiant n'est qu'un numéro d'import : la couche de contenu vise la clé stable.",
        create: Create::No,
        search: Search {
            from: "siege_binding",
            columns: vec![col("id", "id", "ID"), col("region_id", "region_id", "Région"), col_enum("binding_type", "binding_type", "Type", BINDING_TYPE), col("name", "name", "Nom")],
            filters: vec![eq("region_id", "Région", "region_id"), eq_enum("binding_type", "Type", "binding_type", BINDING_TYPE)],
            order: "region_id, binding_type, id",
            group_by: None,
        },
        tabs: vec![single(
            "binding",
            "Point",
            "",
            SingleTab {
                table: "siege_binding",
                key: |id| by("id", id),
                main: true,
                defaults: |_| row(&[]),
                on_copy: None,
                fields: vec![
                    f("id", "ID").int().ro(),
                    f("stable_key", "Clé stable").ro().nullable(),
                    f("region_id", "Région").reference(RefKind::Region).server(),
                    f("name", "Nom"),
                    f("binding_type", "Type").enumeration(BINDING_TYPE).server(),
                    f("pos_x", "X").float().server(),
                    f("pos_y", "Y").float().server(),
                    f("pos_z", "Z").float().server(),
                ],
            },
        )],
    }
}

fn skull_chance() -> Entity {
    Entity {
        route: "pvp-skull-chance",
        label: "Crânes : chance",
        singular: "Écart de niveau",
        group: "Monde",
        db: "world",
        table: "pvp_skull_chance",
        key_col: "level_gap",
        key_label: "Écart (tueur − victime)",
        key_text: false,
        reference: None,
        help: "pvpskulldroptable.txt : chance qu'un joueur tué dans la Battle Square lâche un crâne, selon l'écart de niveau.",
        create: Create::Manual,
        search: Search {
            from: "pvp_skull_chance",
            columns: vec![col("level_gap", "level_gap", "Écart"), col("chance", "chance", "Chance (%)")],
            filters: vec![],
            order: "level_gap",
            group_by: None,
        },
        tabs: vec![single(
            "chance",
            "Chance",
            "",
            SingleTab {
                table: "pvp_skull_chance",
                key: |id| by("level_gap", id),
                main: true,
                defaults: |_| row(&[("chance", json!(0))]),
                on_copy: None,
                fields: vec![f("level_gap", "Écart de niveau").int().ro().server(), f("chance", "Chance (%)").float().server()],
            },
        )],
    }
}

fn skull_level() -> Entity {
    Entity {
        route: "pvp-skull-level",
        label: "Crânes : niveau",
        singular: "Tranche de niveau",
        group: "Monde",
        db: "world",
        table: "pvp_skull_level",
        key_col: "victim_level_min",
        key_label: "Niveau min. de la victime",
        key_text: false,
        reference: None,
        help: "Niveau du crâne lâché (1 à 3, objet choisi par BattleSquare.SkullTids) selon le niveau de la victime.",
        create: Create::Manual,
        search: Search {
            from: "pvp_skull_level",
            columns: vec![col("victim_level_min", "victim_level_min", "Niveau min."), col("victim_level_max", "victim_level_max", "Niveau max."), col("skull_level", "skull_level", "Crâne")],
            filters: vec![],
            order: "victim_level_min",
            group_by: None,
        },
        tabs: vec![single(
            "level",
            "Tranche",
            "",
            SingleTab {
                table: "pvp_skull_level",
                key: |id| by("victim_level_min", id),
                main: true,
                defaults: |_| row(&[("victim_level_max", json!(0)), ("skull_level", json!(1))]),
                on_copy: None,
                fields: vec![f("victim_level_min", "Niveau min.").int().ro().server(), s("victim_level_max", "Niveau max."), s("skull_level", "Niveau du crâne")],
            },
        )],
    }
}

pub fn entities() -> Vec<Entity> {
    vec![teleport(), region(), shrine(), shrine_reward(), skull_chance(), skull_level(), castle(), siege_object(), siege_binding()]
}
