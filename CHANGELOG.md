# Changelog

## 0.3.0 — 2026-11-23

Réécriture complète en Rust, alignée sur le schéma actuel d'ArchCore.

- **Interface 100 % Rust** (egui / eframe) : plus de JavaScript, de serveur local ni de vue web. Un seul
  exécutable, sans dépendance à WebView2 ou WebKitGTK.
- **Moteur en Rust** : modèle déclaratif (`model.rs`, `defs/`), chargement et requêtes (`editor.rs`, `sql.rs`),
  requêtes sur un fil à part. Même SQL qu'avant : changements, entrée complète, `JSON_SET` des seules clés
  modifiées, clés stables.
- **Schéma réel** lu à la connexion : colonnes non décrites affichées avec leur commentaire, tables absentes
  signalées.
- **30 éditeurs** au lieu de 12 : IA des monstres, recettes, compétences de production, ensembles de récolte,
  rangs de butin, titres, rangs de charisme, expérience par niveau, Shrines et leurs récompenses, crânes de la
  Battle Square, châteaux, objets et résurrection de siège, tailles et emblèmes de guilde, pages de banque ;
  onglets ajoutés : récolte, apparence, fenêtre des boss, métamorphose, recette, maîtrises, glossaire.
- Nouvelles colonnes d'ArchCore : `respawn_max_sec`, prix en charisme et en Shrine Coins, `point_id`, règles
  des régions, `gather` des groupes de quêtes, origines des traductions.
- Badge « serveur » revu d'après un relevé complet de ce que lit le serveur (clés JSON comprises).
- Fond de l'écran de connexion ; options `--connect` et `--open`.
- Tests : interface sans fenêtre (egui_kittest), parcours des 30 éditeurs sur une vraie base, écriture sur une
  copie de travail.

## 0.2.0 — 2026-10-22

- **Application de bureau** : ArchForge ouvre sa propre fenêtre (titre, icône, sans barre d'adresse ni
  console) et y affiche l'interface par la vue web du système (WebView2 sous Windows, WebKit sous Linux et
  macOS), comme Keira3 dans Electron. Le navigateur n'est plus utilisé.
- Fermeture de la fenêtre : confirmation s'il reste des modifications non enregistrées.
- Erreurs de démarrage affichées dans une boîte de dialogue.
- Liens vers d'autres sites ouverts dans le navigateur du système.
- Options `--browser` (interface dans le navigateur, si la vue web manque) et `--serve` (sans rien ouvrir) ;
  `--no-browser` reste accepté.
- Écran de connexion : la colonne latérale vide n'apparaît plus.

## 0.1.1 — 2026-10-22

- Lancement d'un double-clic sous Windows : en cas d'erreur, la fenêtre reste ouverte et affiche le message
  (elle se fermait aussitôt).
- Port 8787 déjà pris : ArchForge prend le premier port libre parmi les vingt suivants ; si c'est une autre
  instance d'ArchForge qui l'occupe, sa page est simplement rouverte.

## 0.1.0 — 2026-10-22

Première version.

- Serveur local en Rust (axum, mysql_async) : interface embarquée, API protégée par une clé et limitée à
  `127.0.0.1`, scripts exécutés en transaction, connexion reprise de `config/world.conf` d'ArchCore.
- Couche de contenu : ajout des requêtes à `content/<AAAAMMJJHHMM>_<sujet>.sql`, instructions de schéma refusées.
- Moteur d'édition sur le modèle de Keira3 : onglets une ligne / plusieurs lignes / liste, requêtes
  « changements » et « entrée complète », création par copie, recherche, entrées récentes.
- Colonnes JSON des tables d'ArchLord : clés expliquées, `JSON_SET` / `JSON_REMOVE` des seules clés modifiées.
- Lignes visées par `stable_key` (apparitions, butin, résurrection), identifiant calculé à l'insertion.
- Éditeurs : créatures, PNJ, objets, boutiques, groupes de butin, quêtes, groupes de quêtes, compétences,
  téléportation, régions, langues, comptes ; éditeur SQL ; couche de contenu ; tableau de bord.
