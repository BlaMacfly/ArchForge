//! Guildes, banque, langues et comptes.

use serde_json::{json, Value};

use crate::model::*;

fn s(name: &'static str, label: &'static str) -> Field {
    f(name, label).int().server()
}

const MARK_LAYER: &[(i64, &str)] = &[(1, "Fond"), (2, "Motif"), (3, "Symbole"), (4, "Couleur")];
const TIER: &[(i64, &str)] = &[(1, "Version officielle"), (2, "Client multilingue Webzen 2010"), (3, "Communauté")];

fn guild_size() -> Entity {
    Entity {
        route: "guild-size",
        label: "Tailles de guilde",
        singular: "Taille de guilde",
        group: "Guildes et banque",
        db: "world",
        table: "guild_max_member",
        key_col: "max_members",
        key_label: "Membres max.",
        key_text: false,
        reference: None,
        help: "guildmaxmember.txt : la première ligne donne la taille d'une guilde neuve et le prix de sa création ; les suivantes, le prix de chaque agrandissement.",
        create: Create::Manual,
        search: Search {
            from: "guild_max_member",
            columns: vec![col("max_members", "max_members", "Membres"), col("gold", "gold", "Or"), col("skull_tid", "skull_tid", "Crâne"), col("skull_count", "skull_count", "Nombre")],
            filters: vec![],
            order: "max_members",
            group_by: None,
        },
        tabs: vec![single(
            "size",
            "Taille",
            "",
            SingleTab {
                table: "guild_max_member",
                key: |id| by("max_members", id),
                main: true,
                defaults: |_| row(&[("gold", json!(0)), ("skull_tid", json!(0)), ("skull_count", json!(0))]),
                on_copy: None,
                fields: vec![
                    f("max_members", "Membres max.").int().ro().server(),
                    s("gold", "Prix en or"),
                    f("skull_tid", "Crânes payés").reference(RefKind::Item).server(),
                    s("skull_count", "Nombre de crânes"),
                ],
            },
        )],
    }
}

fn guild_mark() -> Entity {
    Entity {
        route: "guild-mark",
        label: "Emblèmes de guilde",
        singular: "Élément d'emblème",
        group: "Guildes et banque",
        db: "world",
        table: "guild_mark",
        key_col: "tid",
        key_label: "TID",
        key_text: false,
        reference: None,
        help: "guildmark.txt : un emblème est la somme d'un fond, d'un motif et d'un symbole (TID = fond | motif << 8 | symbole << 16), avec une couleur par couche.",
        create: Create::No,
        search: Search {
            from: "guild_mark",
            columns: vec![col("tid", "tid", "TID"), col_enum("layer", "layer", "Couche", MARK_LAYER), col("number", "number", "N°"), col("name", "name", "Nom"), col("gold", "gold", "Or"), col("cash", "cash", "Cash")],
            filters: vec![eq_enum("layer", "Couche", "layer", MARK_LAYER), like("name", "Nom", "name")],
            order: "layer, number",
            group_by: None,
        },
        tabs: vec![single(
            "mark",
            "Élément",
            "",
            SingleTab {
                table: "guild_mark",
                key: |id| by("tid", id),
                main: true,
                defaults: |_| row(&[]),
                on_copy: None,
                fields: vec![
                    f("tid", "TID").int().ro().server(),
                    f("layer", "Couche").enumeration(MARK_LAYER).server(),
                    f("number", "Numéro").int(),
                    f("name", "Nom").server(),
                    s("gold", "Prix en or"),
                    f("skull_tid", "Crânes payés").reference(RefKind::Item).server(),
                    s("skull_count", "Nombre de crânes"),
                    s("cash", "Prix Cash"),
                    f("argb", "Couleur (ARGB)").int(),
                ],
            },
        )],
    }
}

fn bank_slot() -> Entity {
    Entity {
        route: "bank-slot",
        label: "Pages de banque",
        singular: "Page de banque",
        group: "Guildes et banque",
        db: "world",
        table: "bank_slot_price",
        key_col: "slot",
        key_label: "Page",
        key_text: false,
        reference: None,
        help: "bankslotprice.txt : prix d'une page de banque en plus (1 : la deuxième page).",
        create: Create::NextId("SELECT COALESCE(MAX(slot), 0) + 1 AS n FROM bank_slot_price"),
        search: Search { from: "bank_slot_price", columns: vec![col("slot", "slot", "Page"), col("price", "price", "Prix")], filters: vec![], order: "slot", group_by: None },
        tabs: vec![single(
            "slot",
            "Page",
            "",
            SingleTab {
                table: "bank_slot_price",
                key: |id| by("slot", id),
                main: true,
                defaults: |_| row(&[("price", json!(-1))]),
                on_copy: None,
                fields: vec![f("slot", "Page").int().ro().server(), s("price", "Prix").help("-1 : pas en vente contre de l'or.")],
            },
        )],
    }
}

fn language() -> Entity {
    Entity {
        route: "language",
        label: "Langues",
        singular: "Langue",
        group: "Textes",
        db: "world",
        table: "language",
        key_col: "code",
        key_label: "Code BCP 47",
        key_text: true,
        reference: Some(RefKind::Language),
        help: "Langues proposées aux clients ArchRust ; les textes sont dans les onglets « Traductions » de chaque éditeur.",
        create: Create::Manual,
        search: Search {
            from: "language",
            columns: vec![
                col("code", "code", "Code"),
                col("native_name", "native_name", "Nom"),
                col_enum("tier", "tier", "Niveau", TIER),
                col_enum("enabled", "enabled", "Active", super::YES_NO),
                col("(SELECT COUNT(*) FROM localized_text t WHERE t.lang = language.code)", "texts", "Textes"),
            ],
            filters: vec![like("code", "Code", "code")],
            order: "sort_order",
            group_by: None,
        },
        tabs: vec![single(
            "language",
            "Langue",
            "",
            SingleTab {
                table: "language",
                key: |id| by("code", id),
                main: true,
                defaults: |_| row(&[("name_en", json!("")), ("native_name", json!("")), ("tier", json!(3)), ("client_folder", Value::Null), ("enabled", json!(0)), ("sort_order", json!(100))]),
                on_copy: None,
                fields: vec![
                    f("code", "Code").ro().server(),
                    f("name_en", "Nom anglais").server(),
                    f("native_name", "Nom dans la langue").server(),
                    f("tier", "Niveau").enumeration(TIER).server(),
                    f("client_folder", "Dossier de textes du client").nullable().server().help("Game/ini, Game/ini_fr… ; vide : aucun."),
                    f("enabled", "Proposée aux clients").bool().server(),
                    s("sort_order", "Ordre"),
                ],
            },
        )],
    }
}

fn account() -> Entity {
    Entity {
        route: "account",
        label: "Comptes",
        singular: "Compte",
        group: "Comptes",
        db: "auth",
        table: "account",
        key_col: "id",
        key_label: "ID",
        key_text: false,
        reference: Some(RefKind::Account),
        help: "Un compte se crée avec `archcore db-init --account <nom> <mot_de_passe> [niveau_MJ]` (le mot de passe est haché). Le serveur relit le niveau MJ et les bannissements à la connexion.",
        create: Create::No,
        search: Search {
            from: "account",
            columns: vec![col("id", "id", "ID"), col("username", "username", "Nom"), col("gm_level", "gm_level", "Niveau MJ"), col("CAST(last_login AS CHAR)", "last_login", "Dernière connexion")],
            filters: vec![eq("id", "ID", "id"), like("username", "Nom", "username"), eq("gm_level", "Niveau MJ", "gm_level")],
            order: "id",
            group_by: None,
        },
        tabs: vec![
            single(
                "account",
                "Compte",
                "",
                SingleTab {
                    table: "account",
                    key: |id| by("id", id),
                    main: true,
                    defaults: |_| row(&[]),
                    on_copy: None,
                    fields: vec![
                        f("id", "ID").int().ro(),
                        f("username", "Nom").server(),
                        s("gm_level", "Niveau MJ").help("0 joueur ; les niveaux exigés par chaque commande se règlent dans world.conf (GM.CommandLevel.<commande>)."),
                        f("language", "Langue").reference(RefKind::Language).nullable().server().help("Dernier choix du client ArchRust ; vide : World.Language."),
                        f("banned_until", "Banni jusqu'au (ancien champ)").nullable().help("Utiliser l'onglet Bannissements."),
                        f("last_login", "Dernière connexion").ro(),
                        f("last_ip", "Dernière IP").ro(),
                        f("created_at", "Créé le").ro(),
                    ],
                },
            ),
            multi(
                "bans",
                "Bannissements",
                "Un compte est refusé tant qu'un bannissement actif n'est pas échu. Fin vide : sans fin.",
                MultiTab {
                    table: "account_banned",
                    parent: |id| by("account_id", id),
                    order: "id",
                    row_key: vec!["id"],
                    auto_id: None,
                    auto_increment: Some("id"),
                    copy: false,
                    columns: vec![
                        f("id", "N°").int().ro(),
                        f("banned_at", "Le").ro(),
                        f("banned_until", "Jusqu'au").nullable().server().help("AAAA-MM-JJ HH:MM:SS ; vide : sans fin."),
                        f("banned_by", "Par").nullable(),
                        f("reason", "Raison").nullable(),
                        f("active", "Actif").bool().server(),
                    ],
                    new_row: |id, _, _| row(&[("account_id", id.clone()), ("banned_until", Value::Null), ("banned_by", json!("ArchForge")), ("reason", json!("")), ("active", json!(1))]),
                    fresh: None,
                },
            ),
            Tab {
                db: Some("characters"),
                ..list(
                    "characters",
                    "Personnages",
                    "",
                    ListTab {
                        query: |id| {
                            (
                                "SELECT id, slot, name, template_id, level, CAST(last_played AS CHAR) AS last_played, IF(deleted_at IS NULL, '', 'supprimé') AS etat \
                                 FROM characters WHERE account_id = ? ORDER BY slot"
                                    .into(),
                                vec![id.clone()],
                            )
                        },
                        columns: vec![
                            ("id", "ID", None),
                            ("slot", "Emplacement", None),
                            ("name", "Nom", None),
                            ("template_id", "Modèle", Some(RefKind::Creature)),
                            ("level", "Niv.", None),
                            ("last_played", "Dernière partie", None),
                            ("etat", "État", None),
                        ],
                        empty: "Aucun personnage.",
                    },
                )
            },
        ],
    }
}

pub fn entities() -> Vec<Entity> {
    vec![guild_size(), guild_mark(), bank_slot(), language(), account()]
}
