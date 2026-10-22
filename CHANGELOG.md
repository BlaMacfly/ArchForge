# Changelog

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
