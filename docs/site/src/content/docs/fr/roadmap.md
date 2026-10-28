---
title: Feuille de route
description: Ce que Beholder sait faire aujourd'hui, ce qui est en cours et ce qui est prévu.
---

Beholder est construit par phases (voir `docs/SPEC.md`). Cette page est mise à jour à chaque étape terminée.

## Terminé

### Phase 1 : fondations
- Serveur Rust (axum + SQLite) et application de bureau (Tauri 2 + Leptos) pour Linux et Windows.
- Comptes avec nom d'utilisateur et mot de passe, sessions gardées dans le trousseau du système.
- Campagnes : création, invitation par code, nouveau code, image de couverture, titre.
- Règles de campagne : le MJ peut-il lire les notes privées des joueurs, et ce que les joueurs voient en combat.
- Synchronisation pensée pour les mauvaises connexions : modifications enregistrées sur votre ordinateur puis envoyées dès que possible, jamais appliquées deux fois ; fonctionnement hors ligne depuis le cache local ; pastille et bandeau de connexion.
- Interface en anglais et en français, modifiable à tout moment ; toute l'interface s'agrandit avec Ctrl + et Ctrl −.
- Image serveur, Docker Compose avec Caddy, installateurs construits par la CI.

### Phase 2 : notes et documents
- Notes en Markdown avec images.
- Publics : privé, MJ uniquement, tout le groupe, ou membres choisis. Le serveur n'envoie jamais une note hors de son public.
- **Révéler** au groupe en un clic ; un document révélé apparaît aussitôt chez tous.
- Édition à plusieurs pour les notes partagées, avec copies de conflit au lieu de texte perdu.

### Phase 3 : compendium
- **SRD 5.2** (règles 2024) : tous les sorts, monstres, objets, classes, sous-classes, espèces, historiques, dons, aptitudes, traits et états, consultables hors ligne.
- **Livres Open5e :** Kobold Press, Black Flag, Level Up A5E, SRD 5.1 et d'autres, 20 livres en tout.
- Le MJ choisit les sources de règles par campagne ; les joueurs ne téléchargent que celles-là.
- Même nom dans plusieurs sources : affiché une fois selon la priorité, les autres versions sous « Aussi dans ».
- Écran de crédits avec toutes les mentions de licence.
- **Bibliothèque du serveur :** les entrées propres à votre groupe (création maison et contenu des livres que vous possédez), partagées sur votre serveur, avec un éditeur par type d'entrée, l'import/export JSON et YAML et la priorité sur les autres sources. Le MJ peut masquer une entrée aux joueurs d'une campagne.
- **Import par collage :** coller une entrée ou un chapitre entier de sorts, monstres, objets magiques, dons ou sous-classes, relire les entrées détectées avec les doublons signalés, modifier n'importe laquelle et en accepter tout ou partie.
- **Gros imports :** des dizaines de milliers d'entrées par fichier, envoyées par lots, avec l'option de remplacer les entrées déjà présentes, l'annulation d'un import entier en une action (en conservant les entrées modifiées depuis), et les fichiers JSON de toute structure lus via des profils de correspondance enregistrés et exportables, avec un résumé avant l'import. Un convertisseur transforme les fichiers JSON de `dnd-data` en fichiers de bibliothèque, et le format des fichiers de bibliothèque est une norme documentée, avec un schéma JSON.
- **Import à l'installation :** avec `BEHOLDER_IMPORT_REPO`, le serveur importe une fois un dépôt organisé comme dnd-data, en un seul import annulable.

## En cours

Rien pour l'instant : la phase 4 est la suivante.

## Prévu

### Phase 4 : fiches de personnage
- Fiches assistées : l'application calcule modificateurs, maîtrise, sauvegardes, compétences, emplacements de sorts et CA ; vous choisissez espèce, classe, historique et dons dans le compendium.

### Phase 5 : dés
- Jets depuis la fiche, journal de jets partagé, jets secrets du MJ, saisie de dés physiques.

### Phase 6 : suivi de combat et carte de combat
- Initiative, tours, PV et états, avec la santé des monstres exacte, vague ou masquée pour les joueurs.
- **Carte de combat :** une grille carrée avec des formes simples pour commencer (pions, murs, zones). Le MJ contrôle tout : il ajoute, déplace, nomme, supprime, masque ou révèle n'importe quelle forme ; les formes masquées n'arrivent jamais dans l'application des joueurs. Chaque joueur ne déplace que le pion de son personnage actif, et chaque joueur a sa propre couleur, avec les initiales du personnage sur le pion pour que la couleur ne soit pas le seul repère. Les pions peuvent être liés aux combattants du suivi.
- Plus tard pour la carte : images de fond, brouillard de guerre, mesure et gabarits de zone, images de pions.

### Phase 7 : connexion Discord
- Se connecter avec Discord et le lier à un compte existant.

### Plus tard
- Application Android.
- Un bot Discord qui lance les dés avec les modificateurs de la fiche, synchronisé avec l'application.
- D'autres systèmes de jeu (par ex. Imperium Maledictum, L'Appel de Cthulhu) ; chaque source est déjà marquée de son système.
- Des systèmes de jeu maison définis dans l'application.
- Des scripts pour personnaliser le comportement d'une campagne.
- Un système de mods et d'extensions pour toute l'application.

## Limites connues

- Les liens des notes et du compendium ne sont pas encore cliquables.
- Les notes d'une campagne doivent tenir dans un message de synchronisation de 4 Mo (plusieurs centaines de notes habituelles).
- Les notes tapées hors ligne sont mises en file avec un enregistrement par pause de frappe.
- Les entrées du glossaire des règles (propriétés d'armes, bottes d'armes, compétences) ne sont pas encore dans le compendium.
- Les mentions OGL des livres Open5e sont générées à partir de leurs métadonnées, pas copiées des livres.
- Une mise à jour de l'application peut réinitialiser son cache local et perdre les modifications encore en attente hors ligne à ce moment.
- L'export de la bibliothèque enregistre dans le dossier Téléchargements ; il n'y a pas encore de fenêtre d'enregistrement.
- Les descriptions converties depuis `dnd-data` retrouvent leurs tables, titres et listes quand la présentation est reconnue ; le reste reste en texte aplati. Dans ces fichiers, les blocs de statistiques des monstres n'ont ni CA, ni points de vie, ni caractéristiques.
- Les fichiers `dnd-data` ne contiennent presque pas de règles de sous-classes : seules les 12 sous-classes des Free Basic Rules deviennent des entrées de sous-classe.
- Masquer une entrée ne fonctionne que pour la bibliothèque du serveur, pas pour le SRD ni les livres Open5e.

## Tailles

Mesurées sous Linux, compilations de production.

| Élément | Taille |
|---|---:|
| Interface (WASM, après `wasm-opt -Oz`) | 1,05 Mo (426 Ko compressé) |
| Application de bureau (binaire Linux) | 6,1 Mo |
| Téléchargement du SRD 5.2 | 200 Ko |
| Les 20 livres Open5e | 1,6 Mo |
| RAM du serveur, au repos / en pointe (envoi d'une image de 40 MP) | 6 Mo / 157 Mo |
