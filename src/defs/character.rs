//! Personnages : compétences (modèle, nature, niveaux, maîtrises), titres, rangs de charisme, expérience.

use serde_json::json;

use super::leak;
use crate::localized_tab;
use crate::model::*;

fn s(name: &'static str, label: &'static str) -> Field {
    f(name, label).int().server()
}

const MASTERY_TREE: &[(i64, &str)] = &[(1, "Compétences supérieures"), (2, "Compétences héroïques")];

fn spec_sections() -> Vec<Section> {
    vec![section(
        "Lues par le serveur",
        vec![
            s("target", "Cible").help("Unités 1 offensive ; dizaines 1 sur un ami ; centaines 1 sur soi aussi."),
            s("range", "Forme de la zone").help("Au-delà de 1000 : centrée sur la cible ; centaines 3 sphère, 4 boîte ; 303 groupe ; 701 et 702 visée au sol."),
            s("attribute", "Attribut").help("Dizaines 1 renforcement, 2 affaiblissement ; unités 1 à 6 dégâts, 8 production, 9 passive."),
            s("melee_attack", "Attaque").help("101, 102, 103, 108, 109, 110 dégâts immédiats ; 106, 109, 110 critique ; 107 Death Strike."),
            f("special_status", "États spéciaux").server().help(
                "Codes « ; » : 1 étourdissement, 2 gel, 3 ralenti, 4 invincible, 5 invincible aux attributs, 6 pas d'aggro, 7 aggro cachée, \
                 8 protection contre l'étourdissement, 9 halte, 10 invisible, 11 semi-transparent, 12 et 13 invincible aux coups normaux / aux \
                 compétences, 14 compétences interdites, 15 attaque interdite, 16 sommeil, 17 protection étourdissement + ralenti, 18 désarmement, \
                 19 discussion interdite, 20 immobilisé, 21 confusion, 22 peur, 23 maladie, 24 berserk, 25 rétrécissement (≥ 1000 et 26 à 33 ignorés).",
            ),
            s("disturb", "Gêne").help("Modulo 1000, bits : 1 attaque, 2 déplacement, 4 objet, 8 compétence."),
            s("reflect_melee_attack", "Renvoi").help("1 renvoi, 2 bouclier ; bits 0x4 ignore la défense, 0x8 ignore les résistances."),
            s("defense_melee_attack", "Défense").help("2 contre-attaque, 4 résistance aux critiques."),
            s("action_on_action", "Enchaînement").help("1 Time Attack, 2 téléportation, 3 sur l'attaquant, 4 premier coup, 6 explosion, 7 si la cible porte l'effet, 8 si le lanceur est invisible."),
            s("convert_type", "Conversion").help("1 coup reçu changé en PV, 2 en PM, 4 absorption, 6 Vampiric Impact."),
            s("divide", "Partage").help("Bits : 0x1 élémentaire, 0x2 physique."),
            s("dispel_magic", "Purge"),
            s("applied_cooldown_cap", "Réduction de relance max. (%)"),
            s("skill_levelup", "Gain de niveau").help("1 toutes les compétences, 2 arbres de maîtrise, 4 liste."),
            s("upgradable", "Améliorable").help("1 ou 3 : améliorable."),
            s("auto_attack", "Attaque automatique"),
            s("summons", "Invocation").help("1 combat, 2 fixe, 4 limite relevée, 5 soin des invocations, 6 parchemin, 8 familier."),
            s("game_effect", "Effet de jeu").help("1 jour/nuit, 2 pluie, 4 neige."),
            s("ride", "Monture"),
            s("transform_target", "Transformation").help("4 Mutation, 5 évolution du Scion."),
            s("downtime_id", "Immunité (id)"),
            s("downtime_duration", "Immunité (s)"),
            s("requirement", "Exigence").help("1 et 2 arme, 3 bouclier, 4 outil de production, 5 à 19 types d'arme."),
            s("arg1", "Outil exigé").help("Quand requirement vaut 4."),
            s("move_pos", "Déplacement").help("2 le lanceur, 3 la cible."),
            s("at_field", "Zone au sol").help("10001 et 10002 attaque, 10003 aura."),
            s("detect", "Détection"),
            s("resurrection", "Résurrection"),
            s("target_restriction", "Cibles interdites").help("Bits : 1 objet de siège, 2 monstre, 4 joueur ennemi."),
        ],
    )]
}

fn level_sections() -> Vec<Section> {
    let classes = ["knight", "archer", "wizard", "berserker", "hunter", "sorcerer", "swashbuckler", "ranger", "elementaler", "slayer", "obiter", "summoner", "scion"];
    let class_keys: Vec<Field> = classes.iter().map(|c| s(leak(format!("require_class_{c}")), leak(format!("Classe : {c}")))).collect();
    let mut damage = vec![s("damage_a", "Dégâts"), s("damage_a_PERCENT", "Dégâts (%)"), s("damage_a2", "Second coup"), s("damage_a_PERCENT2", "Second coup (%)")];
    for e in ["magic", "water", "fire", "earth", "air", "poison", "thunder", "ice"] {
        damage.push(s(leak(format!("damage_{e}")), leak(format!("Dégâts {e}"))));
        damage.push(s(leak(format!("damage_{e}_PERCENT")), leak(format!("Dégâts {e} (%)"))));
    }
    vec![
        section(
            "Conditions et coûts",
            vec![
                s("require_level", "Niveau requis"),
                s("require_point", "Points requis"),
                s("require_heroic_point", "Points héroïques requis"),
                s("require_charisma_point", "Charisme requis"),
                s("skill_cost", "Coût d'apprentissage"),
                s("skill_upgrade_cost", "Coût d'amélioration"),
                s("cost_hp", "Coût en PV"),
                s("cost_mp", "Coût en PM"),
                s("cost_arrow", "Flèches"),
                s("cost_mp_decrease", "Réduction du coût en PM (%)"),
            ],
        ),
        section(
            "Temps et portée",
            vec![
                s("cast_time", "Incantation (ms)"),
                s("recast_time", "Recharge (ms)"),
                s("duration", "Durée (ms)"),
                s("interval", "Intervalle (ms)"),
                s("stun_time", "Étourdissement (ms)"),
                s("additional_duration", "Durée ajoutée (s)"),
                s("range", "Portée (m)").help("En mètres ; × 100 en unités du jeu."),
                s("area_r", "Rayon de zone (m)"),
                s("area_f1", "Zone avant 1 (m)"),
                s("area_f2", "Zone avant 2 (m)"),
                s("max_target_num", "Cibles max."),
            ],
        ),
        section(
            "Effet",
            vec![
                s("skill_rate", "Taux"),
                s("skill_rate2", "Taux 2"),
                s("critical", "Critique"),
                s("special_status_rate", "Chance d'état"),
                s("death", "Mort (%)"),
                s("buff_type", "Affaiblissement").help("1 : affaiblissement."),
                s("move_distance", "Distance de déplacement"),
                s("skill_type", "Type").help("9 : compétence de l'ArchLord."),
                s("summons_tid", "Invocation (TID)"),
                s("summons_count", "Invocations"),
                s("damage_convert_hp", "Converti en PV"),
                s("damage_convert_mp", "Converti en PM"),
                s("bonus_exp", "Bonus d'expérience"),
                s("bonus_drop_rate", "Bonus de butin"),
                s("bonus_money", "Bonus d'or"),
            ],
        ),
        section("Dégâts", damage),
        section("Classes", class_keys),
    ]
}

fn skill() -> Entity {
    Entity {
        route: "skill",
        label: "Compétences",
        singular: "Compétence",
        group: "Personnages",
        db: "world",
        table: "skill_template",
        key_col: "tid",
        key_label: "TID",
        key_text: false,
        reference: Some(RefKind::Skill),
        help: "Le serveur lit la nature (SKILL_SPEC) et les valeurs par niveau (SKILL_CONST) ; le modèle ne sert qu'au nom.",
        create: Create::NextId("SELECT COALESCE(MAX(tid), 0) + 1 AS n FROM skill_template"),
        search: Search {
            from: "skill_template t LEFT JOIN skill_spec s ON s.tid = t.tid",
            columns: vec![
                col("t.tid", "tid", "TID"),
                col("t.name", "name", "Nom"),
                col("JSON_VALUE(s.spec, '$.range')", "range", "Zone"),
                col("(SELECT COUNT(*) FROM skill_level l WHERE l.tid = t.tid)", "levels", "Niveaux"),
            ],
            filters: vec![eq("tid", "TID", "t.tid"), like("name", "Nom", "t.name")],
            order: "t.tid",
            group_by: None,
        },
        tabs: vec![
            single(
                "template",
                "Modèle",
                "",
                SingleTab {
                    table: "skill_template",
                    key: |id| by("tid", id),
                    main: true,
                    defaults: |_| row(&[("name", json!("Nouvelle compétence")), ("raw", json!({}))]),
                    on_copy: None,
                    fields: vec![f("tid", "TID").int().ro(), f("name", "Nom").server(), f("raw", "Clés de SKILLTEMPLATE.ini").json(Vec::new())],
                },
            ),
            single(
                "spec",
                "Nature",
                "SKILL_SPEC.txt : cible, zone, attribut, états… (colonnes non vides).",
                SingleTab {
                    table: "skill_spec",
                    key: |id| by("tid", id),
                    main: false,
                    defaults: |_| row(&[("name", json!("Nouvelle compétence")), ("spec", json!({}))]),
                    on_copy: None,
                    fields: vec![
                        f("tid", "TID").int().ro(),
                        f("name", "Nom").server().help("Sert à reconnaître certaines compétences par leur nom."),
                        f("spec", "Colonnes de SKILL_SPEC.txt").json(spec_sections()),
                    ],
                },
            ),
            multi(
                "levels",
                "Niveaux",
                "SKILL_CONST.txt : valeurs de chaque niveau (bouton « Modifier »). Les niveaux doivent se suivre depuis 1.",
                MultiTab {
                    table: "skill_level",
                    parent: |id| by("tid", id),
                    order: "level",
                    row_key: vec!["tid", "level"],
                    auto_id: None,
                    auto_increment: None,
                    copy: true,
                    columns: vec![s("level", "Niveau"), f("consts", "Valeurs").json(level_sections()).server()],
                    new_row: |id, rows, last| {
                        let level = rows.iter().filter_map(|r| r.get("level").and_then(|v| v.as_i64())).max().unwrap_or(0) + 1;
                        let consts = last.and_then(|l| l.get("consts").cloned()).unwrap_or(json!({}));
                        row(&[("tid", id.clone()), ("level", json!(level)), ("consts", consts)])
                    },
                    fresh: None,
                },
            ),
            multi(
                "mastery",
                "Maîtrises",
                "SkillMastery_High.txt et _Heroic.txt : place de la compétence dans l'arbre d'une classe et conditions pour l'apprendre.",
                MultiTab {
                    table: "skill_mastery",
                    parent: |id| by("skill_tid", id),
                    order: "class_tid",
                    row_key: vec!["class_tid", "skill_tid"],
                    auto_id: None,
                    auto_increment: None,
                    copy: false,
                    columns: vec![
                        f("class_tid", "Classe (modèle)").reference(RefKind::Creature).server().help("Modèle de la classe ; 10000 + race : arbre d'une race."),
                        f("tree", "Arbre").enumeration(MASTERY_TREE).server(),
                        f("requires", "Compétences exigées").pair_list(Some(RefKind::Skill), "Compétence", "Niveau").server(),
                        f("class_points", "Niveaux par classe").pair_list(Some(RefKind::Creature), "Classe", "Niveaux").server(),
                        s("total_points", "Points en tout").help("Niveaux cumulés de toutes les compétences (arbre supérieur) ou points héroïques dépensés."),
                    ],
                    new_row: |id, _, _| row(&[("class_tid", json!(0)), ("skill_tid", id.clone()), ("tree", json!(1)), ("requires", json!([])), ("class_points", json!([])), ("total_points", json!(0))]),
                    fresh: None,
                },
            ),
            localized_tab!("skill_name"),
        ],
    }
}

const TITLE_CHECKS: &str = "Conditions [type, Set, Value] (5 au plus) : 1 objet ramassé, 10001 récolte, 30001 créature tuée, 40001 objet \
    utilisé, 90001 joueur tué, 140001 niveau, 170001 quête, 200001 / 200002 / 200003 Alchimie / Cuisine / Collecte.";
const TITLE_EFFECTS: &str = "Effets [type, Set, V1, V2] : 1 expérience et 2 or des quêtes (+V1 %), 3 / 4 / 5 réussite Alchimie / Cuisine / \
    Collecte, 6 sertissage, 7 objets récoltés × V2, 8 expérience de la créature V1, 9 potion (+V2 points si Set 0, % si Set 1).";

fn title() -> Entity {
    Entity {
        route: "title",
        label: "Titres",
        singular: "Titre",
        group: "Personnages",
        db: "world",
        table: "title_template",
        key_col: "tid",
        key_label: "TID",
        key_text: false,
        reference: Some(RefKind::Title),
        help: "titledatatable.txt (épisode 8) : conditions, coût et effets d'un titre. Les listes sont en JSON.",
        create: Create::NextId("SELECT COALESCE(MAX(tid), 0) + 1 AS n FROM title_template"),
        search: Search {
            from: "title_template",
            columns: vec![col("tid", "tid", "TID"), col("name", "name", "Nom"), col("category", "category", "Catégorie"), col("require_level", "require_level", "Niveau")],
            filters: vec![eq("tid", "TID", "tid"), like("name", "Nom", "name"), like("category", "Catégorie", "category")],
            order: "tid",
            group_by: None,
        },
        tabs: vec![single(
            "title",
            "Titre",
            "",
            SingleTab {
                table: "title_template",
                key: |id| by("tid", id),
                main: true,
                defaults: |_| {
                    row(&[
                        ("name", json!("Nouveau titre")),
                        ("category", json!("")),
                        ("cost_items", json!([])),
                        ("first_titles", json!([])),
                        ("checks", json!([])),
                        ("skills", json!([])),
                        ("effects", json!([])),
                    ])
                },
                on_copy: None,
                fields: vec![
                    f("tid", "TID").int().ro().server(),
                    f("name", "Nom").server(),
                    f("category", "Catégorie").server(),
                    f("category_id", "Catégorie (liste)").int().help("FirstCategory de titlestringtable.txt : 200 Expert Collector, 500 Addict, 1500 Advanced, 2100 Researcher."),
                    f("optional", "Facultatif").bool().server().help("1 : chargé seulement avec Title.Schola."),
                    s("require_level", "Niveau").group("Conditions"),
                    s("require_class", "Classe").group("Conditions").help("0 tous ; 101 chevalier … 1001 Dragon Scion ; 1101-1301 formes évoluées."),
                    f("require_guild_master", "Chef de guilde").bool().server().group("Conditions"),
                    f("require_archlord", "ArchLord").bool().server().group("Conditions"),
                    s("require_con", "Constitution").group("Conditions"),
                    s("require_str", "Force").group("Conditions"),
                    s("require_int", "Intelligence").group("Conditions"),
                    s("require_dex", "Dextérité").group("Conditions"),
                    s("require_wis", "Sagesse").group("Conditions"),
                    s("require_cha", "Charisme").group("Conditions"),
                    s("cost_geld", "Coût en or").group("Coût"),
                    f("cost_cash", "Coût Cash").int().group("Coût").help("Vide dans les données, ignoré."),
                    f("cost_items", "Objets payés").pair_list(Some(RefKind::Item), "Objet", "Quantité").server(),
                    f("first_titles", "Titres préalables").list(Some(RefKind::Title)).server(),
                    f("checks", "Conditions").list(None).server().help(TITLE_CHECKS),
                    f("skills", "Compétences données").list(Some(RefKind::Skill)).server(),
                    f("effects", "Effets").list(None).server().help(TITLE_EFFECTS),
                ],
            },
        )],
    }
}

fn charisma_title() -> Entity {
    Entity {
        route: "charisma-title",
        label: "Rangs de charisme",
        singular: "Rang de charisme",
        group: "Personnages",
        db: "world",
        table: "charisma_title",
        key_col: "rank",
        key_label: "Rang",
        key_text: false,
        reference: None,
        help: "charismatitletable.txt : le client calcule le rang d'après le charisme envoyé ; le serveur ne s'en sert que pour /title.",
        create: Create::NextId("SELECT COALESCE(MAX(`rank`), 0) + 1 AS n FROM charisma_title"),
        search: Search {
            from: "charisma_title",
            columns: vec![col("`rank`", "rank", "Rang"), col("title", "title", "Titre"), col("charisma_min", "charisma_min", "Min."), col("charisma_max", "charisma_max", "Max.")],
            filters: vec![],
            order: "`rank`",
            group_by: None,
        },
        tabs: vec![single(
            "rank",
            "Rang",
            "",
            SingleTab {
                table: "charisma_title",
                key: |id| by("rank", id),
                main: true,
                defaults: |_| row(&[("charisma_min", json!(0)), ("charisma_max", json!(0)), ("title", json!(""))]),
                on_copy: None,
                fields: vec![f("rank", "Rang").int().ro(), s("charisma_min", "Charisme min."), s("charisma_max", "Charisme max."), f("title", "Titre").server()],
            },
        )],
    }
}

fn level_exp() -> Entity {
    Entity {
        route: "level-exp",
        label: "Expérience par niveau",
        singular: "Niveau",
        group: "Personnages",
        db: "world",
        table: "level_exp",
        key_col: "level",
        key_label: "Niveau",
        key_text: false,
        reference: None,
        help: "LevelUpExp.txt : expérience à atteindre pour chaque niveau.",
        create: Create::NextId("SELECT COALESCE(MAX(level), 0) + 1 AS n FROM level_exp"),
        search: Search { from: "level_exp", columns: vec![col("level", "level", "Niveau"), col("exp", "exp", "Expérience")], filters: vec![eq("level", "Niveau", "level")], order: "level", group_by: None },
        tabs: vec![single(
            "level",
            "Niveau",
            "",
            SingleTab {
                table: "level_exp",
                key: |id| by("level", id),
                main: true,
                defaults: |_| row(&[("exp", json!(0))]),
                on_copy: None,
                fields: vec![f("level", "Niveau").int().ro().server(), s("exp", "Expérience")],
            },
        )],
    }
}

pub fn entities() -> Vec<Entity> {
    vec![skill(), title(), charisma_title(), level_exp()]
}
