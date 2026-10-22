<p align="center">
  <img src="web/img/logo.png" alt="ArchForge" width="260">
</p>

<h1 align="center">ArchForge</h1>

<p align="center">
  <b>Éditeur de la base de données d'<a href="https://github.com/BlaMacfly/ArchCore">ArchCore</a>, l'émulateur de serveur ArchLord</b><br>
  Rust · MariaDB · interface web locale · Windows, Linux, macOS
</p>

ArchForge crée et modifie le contenu du monde d'ArchCore sans écrire de SQL : créatures et leurs
apparitions, butin, PNJ et leurs fonctions, objets, boutiques, quêtes, compétences, téléportation,
régions, traductions, comptes. Chaque champ est expliqué, des sélecteurs aident à choisir les valeurs et
les drapeaux, et la requête SQL produite est affichée — les seuls changements, ou l'entrée complète pour une
sauvegarde — à copier, à exécuter sur la base ou à **ajouter à la couche de contenu** d'ArchCore.

C'est l'« Éditeur de la base de données » de la [feuille de route d'ArchCore](https://github.com/BlaMacfly/ArchCore/wiki/FR-Feuille-de-route),
sur le modèle de [Keira3](https://github.com/azerothcore/Keira3) (AzerothCore).

## 1. Ce que fait Keira3 (analyse)

Keira3 est l'éditeur de base de données d'AzerothCore (WoW 3.3.5) : Angular 22, Electron 41, Bootstrap,
monorepo Nx, `mysql2` (avec tunnel SSH), `squel` pour construire le SQL, SQLite pour les données du client
(sorts…). Son fonctionnement tient en quelques mécanismes :

| Mécanisme | Dans Keira3 |
|---|---|
| **Éditeur générique** | des classes de base (`SingleRowEditorService`, `MultiRowEditorService`, `…ComplexKey…`) : un éditeur concret ne déclare que sa table, sa clé et sa classe de ligne ; le formulaire, le rechargement et le SQL sont hérités |
| **SQL produit** | `MysqlQueryService` : `getUpdateQuery` (seulement les colonnes changées), `getDiffDeleteInsert…` (lignes ajoutées ou retirées), `getFullDeleteInsertQuery` (entrée complète) ; affichés dans un panneau « Diff / Full » avec Copier et Exécuter |
| **Entité et onglets** | un *handler* par entité (créature, quête, objet, objet de jeu…) et un onglet par table rattachée (template, spawn, loot, equip…) ; un point signale l'onglet modifié |
| **Recherche et création** | page de recherche par filtres ; création avec un nouvel ID libre ou par copie d'une entrée existante |
| **Aide à la saisie** | sélecteurs (recherche d'un objet, d'une créature, d'un sort…), fenêtres de drapeaux (bits), listes de valeurs, info-bulles par champ |
| **Outils** | éditeur SQL libre, éditeur SmartAI, conditions, textes, tableau de bord, visionneuse de carte et de modèles 3D |
| **Connexion** | fenêtre de connexion, connexions récentes, base publique en lecture seule |

## 2. Transposition à ArchCore

| Keira3 (AzerothCore) | ArchForge (ArchCore) |
|---|---|
| Electron + Angular, un installeur par système | **un seul exécutable Rust** (comme `archcore`) qui sert l'interface embarquée sur `127.0.0.1` et ouvre le navigateur ; interface en JavaScript sans étape de compilation |
| connexion saisie à la main | reprise de `config/world.conf` du dépôt ArchCore (`--archcore`) : hôte, compte, noms des quatre bases |
| SQL exécuté sur la base | exécuté **dans une transaction** (tout ou rien) — *et* : **« Ajouter au contenu »** écrit la requête dans `content/<AAAAMMJJHHMM>_<sujet>.sql`. `archcore import` régénère la base monde depuis le client : sans cette couche, toute modification serait perdue au prochain import |
| colonnes typées | ArchLord garde ses tables d'origine en **JSON** (`raw`, `data`, `spec`, `consts`) : les clés utiles sont des champs expliqués, les autres restent modifiables ; le « diff » n'écrit que les clés changées (`JSON_SET` / `JSON_REMOVE`), si bien qu'un fichier de contenu ne recopie pas les données du client |
| clés primaires stables | `monster_spawn`, `monster_drop`, `resurrection_point` n'ont qu'un numéro d'import : les lignes sont visées par leur **`stable_key`** ; une ligne ajoutée reçoit une clé `…#af…` et un `id` calculé au passage (`MAX(id) + 1`), ce qui rend la requête rejouable |
| `creature_template` + `creature` + `creature_loot_template` | `character_template` + `character_data_table` + `monster_spawn` + `monster_drop` + `growup_factor` |
| `npc_vendor`, `gossip` | `npc_trade_list` et les **fonctions de `npc.ini`** (paires ordonnées : dialogue, boutique, quêtes, enchères), éditées en place |
| `quest_template`, `creature_queststarter` | `quest_template.data` (conditions, objectifs, récompenses lus par le serveur) et `quest_group` |
| `*_locale` | `localized_text` (onglet « Traductions » de chaque éditeur) et `language` |
| champs documentés | en plus, un badge **« serveur »** sur chaque champ que lit vraiment ArchCore (relevé dans `crates/archcore/src/world/*.rs`) |

Les éditeurs :

| Éditeur | Tables |
|---|---|
| **Créatures** | `character_template`, `character_data_table`, `monster_spawn`, `monster_drop`, `growup_factor`, traductions |
| **PNJ** | `npc_spawn` (position, modèle, fonctions de `npc.ini`), traductions |
| **Objets** | `item_data`, `item_template`, où l'objet est vendu, qui le lâche, traductions |
| **Boutiques** | `npc_trade_list`, PNJ vendeurs |
| **Groupes de butin** | `drop_group`, objets du groupe |
| **Quêtes**, **Groupes de quêtes** | `quest_template`, `quest_group`, PNJ du groupe, traductions |
| **Compétences** | `skill_template`, `skill_spec`, `skill_level`, traductions |
| **Téléportation** | `teleport_point`, `teleport_fee`, portails |
| **Régions** | `region_template`, `resurrection_point`, traductions |
| **Langues**, **Comptes** | `language` ; `account`, `account_banned`, personnages du compte |
| **Éditeur SQL**, **Couche de contenu** | requêtes libres ; fichiers de `content/` |

## 3. Installation et lancement

Prérequis : [Rust](https://rustup.rs) 1.85 ou plus et une base ArchCore (MariaDB) créée par `archcore db-init`
et remplie par `archcore import`.

```bash
cargo build --release
target/release/archforge --archcore ../ArchCore      # ouvre http://127.0.0.1:8787/
```

| Option | Rôle |
|---|---|
| `--archcore <dossier>` | dépôt ArchCore : connexion lue dans `config/world.conf`, couche de contenu dans `content/`. Par défaut : le dossier courant s'il contient `config/world.conf`, sinon la variable `ARCHCORE_DIR` |
| `--port <n>` | port local (8787 par défaut) |
| `--no-browser` | ne pas ouvrir le navigateur |

Sous Windows, l'exécutable porte l'icône d'ArchForge (`assets/archforge.ico`, intégrée par `build.rs`).

## 4. Utilisation

1. **Connexion** : les valeurs de `world.conf` sont proposées ; les connexions récentes sont gardées dans le
   navigateur (le mot de passe seulement si on le demande).
2. **Rechercher** une entrée par ses filtres, ou en **créer** une : identifiant libre proposé, copie possible
   d'une entrée existante (tables rattachées comprises quand cela a un sens : butin, boutique, niveaux…).
3. **Modifier** : chaque onglet montre sous le formulaire la requête produite, en deux formes :
   - *Changements* : seulement ce qui diffère de la base — c'est la forme à mettre dans la couche de contenu ;
   - *Entrée complète* : `DELETE` puis `INSERT`, pour sauvegarder ou transporter une entrée.
4. **Exécuter** (un onglet, ou « Tout enregistrer ») ou **Ajouter au contenu** (un onglet, ou « Tout ajouter au
   contenu ») : nouveau fichier ou ajout à un fichier existant de `content/`.
5. Redémarrer le serveur ArchCore : il charge le monde au démarrage. `archcore content` rejoue la couche de
   contenu seule, sans réimporter.

Les champs en pointillés sont des clés absentes de la ligne (une saisie les ajoute) ; un champ en rouge bloque
l'exécution ; un point orange signale un onglet modifié, et quitter l'entrée demande confirmation.

## 5. Sécurité

L'API exécute le SQL qu'on lui envoie : elle n'écoute que sur `127.0.0.1`, refuse tout en-tête `Host` non local
(rattachement DNS) et exige une clé tirée à chaque lancement, glissée dans la page (une autre page ouverte dans
le navigateur ne peut pas la lire). La couche de contenu refuse `CREATE`, `ALTER`, `DROP`, `TRUNCATE`… (le
schéma passe par `sql/migrations/` d'ArchCore) et n'écrit que des fichiers `AAAAMMJJHHMM_sujet.sql`.

## 6. Organisation du code

```text
src/
  main.rs        options, lancement du serveur local, ouverture du navigateur
  api.rs         API (/api/…), clé et contrôle de l'hôte, fichiers de l'interface embarqués
  db.rs          MariaDB : alias des bases, lectures paramétrées, scripts en transaction
  content.rs     couche de contenu : noms, instructions refusées, ajout aux fichiers
  archcore.rs    lecture de config/world.conf
web/
  js/sql.js      génération du SQL (changements, entrée complète, JSON_SET, clés stables)
  js/editor.js   moteur : onglets une ligne / plusieurs lignes / liste, sortie SQL, recherche, création
  js/fields.js   champs : nombres, listes, drapeaux, références, clés JSON, paires de npc.ini
  js/refs.js     références entre tables : noms affichés, fenêtres de recherche
  js/defs/*.js   un fichier par domaine (créatures, PNJ, objets, quêtes…), purement déclaratif
  js/pages.js    connexion, tableau de bord, éditeur SQL, couche de contenu
tests/sql.test.mjs   tests du générateur SQL
```

### Ajouter un éditeur ou un champ

Un éditeur est une déclaration dans `web/js/defs/` : table principale, clé, recherche (colonnes et
filtres), onglets. Un onglet `single` (une ligne) donne ses champs ; un onglet `multi` (lignes rattachées)
donne `parent`, `rowKey` (colonnes qui désignent une ligne de façon stable) et, au besoin, `autoId` ; un onglet
`list` donne une requête en lecture seule. Un champ : `{ name, label, help, type, enum, flags, ref, server }`.
Ajouter l'éditeur à `web/js/defs/index.js`.

## 7. Tests

```bash
cargo test                       # serveur : noms de contenu, instructions refusées, hôtes, world.conf
node --test tests/sql.test.mjs   # générateur SQL
```

**Essai (2026-10-22)** : MariaDB 13.0 locale, copie de la base monde d'ArchCore. Vérifiés dans le navigateur :
connexion par `world.conf`, tableau de bord, les douze éditeurs et leurs recherches, ajout d'un butin
(sélecteur d'objet, insertion avec clé stable et `MAX(id) + 1`), modification d'une clé JSON (`JSON_SET` d'une
seule clé), création d'un objet par copie (deux tables), boutique (mise à jour, suppression, ajout), fonction de
PNJ modifiée en place, ajout à un fichier de `content/`, transaction annulée sur erreur, refus d'un `DROP` et
d'un appel sans clé. Non essayé : relecture des modifications par un serveur ArchCore redémarré.

## Licence

GPL-3.0-or-later, comme ArchCore et ArchRust. Keira3 (AGPL-3.0) a servi de modèle : aucun code n'en est repris,
seul le principe l'est.
