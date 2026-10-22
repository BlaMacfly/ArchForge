# Changelog

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
