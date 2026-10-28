---
title: Décisions de conception
description: Résumé des ADR de docs/decisions.
sidebar:
  order: 5
---

Les textes complets sont dans `docs/decisions/` (en anglais). Chacun couvre le contexte, la décision, les alternatives et les conséquences.

| ADR | Décision |
|---|---|
| 0001 | Organisation du workspace, noms `beholder-*`, toolchain figée ; ajout de `beholder-ipc` et `beholder-e2e` ; le dossier local `data/` n'est jamais commité. |
| 0002 | `rusqlite` aussi côté serveur, au lieu de sqlx : un seul SQLite compilé, des builds plus rapides, une connexion sérialisée. |
| 0003 | Format réseau : octet de flags + postcard, zstd au-delà de 1 Ko, plafond de 4 Mio, version dans `Hello`, évolution en ajout seul. |
| 0004 | Synchro : journal d'événements par campagne, filtrage par destinataire via `Outgoing`, déduplication par UUIDv7 pendant 8 jours, repli sur instantané, vue optimiste = confirmé + en attente. |
| 0005 | Authentification : Argon2id (19 Mio), jetons opaques hachés, renouvellement avec période de grâce, limites de débit, inscription ouverte. |
| 0006 | Images : contrôle de l'en-tête avant décodage, décodage plafonné, WebP avec perte en miniature + taille d'affichage, adressage par contenu. |
| 0007 | Réseau client : TLS du système (native-tls), `ureq` pour HTTPS, états de connexion et attente progressive. |
| 0008 | Bureau : pas de routeur, IPC Tauri sans surcouche, envoi brut des fichiers, cache d'images `img://`, Fluent, trousseau du système. |
| 0009 | Déploiement : image musl statique sur `scratch`, limites mémoire dans compose, Caddy en HTTP/1+2, installateurs construits par la CI. |
| 0010 | Documentation avec Astro Starlight, en local pour l'instant, sur un Node installé dans le dossier utilisateur. |
| 0011 | Notes : filtrage et partage des trames par utilisateur, révélation par ajout/suppression, rattrapage par instantané quand la visibilité change, copies de conflit, protocole v2, Markdown assaini avec pulldown-cmark. |
| 0012 | Compendium : SRD copié depuis 5e-srd-api (5e-database est archivé), convertisseur `beholder-xtask` réservé au développement, paquet reproductible versionné, servi immuable avec Range, recherche FTS5 sur le client. |
| 0013 | Open5e : un paquet par livre converti depuis un commit fixé (données brutes non copiées), mentions OGL générées, sources par campagne (SRD compris), étiquette de système de jeu, priorité en cas de nom en double avec « Aussi dans », protocole v3. |
| 0014 | Bibliothèque du serveur : modification par l'auteur ou tout MJ, enregistrements périmés refusés, avis par WebSocket et modifications par HTTP, bibliothèque comme source prioritaire, entrées masquées par campagne (protocole v4), fichiers JSON/YAML avec serde_norway. |
| 0015 | Gros imports de bibliothèque : lots SaveMany de 500, paquet de la bibliothèque écrit en flux sur disque et reconstruit seulement au-delà de 1 000 modifications de retard, import du fichier en corps brut, convertisseur dnd-data, format de fichier versionné avec un schéma JSON. |
| 0016 | Imports de bibliothèque annulables : un ImportId par import envoyé avec des lots ImportMany, le serveur note la position précédente de chaque entrée dans le journal, UndoImport supprime les entrées créées et restaure celles remplacées mais garde celles modifiées depuis, par l'auteur de l'import ou n'importe quel MJ. |
| 0017 | Import de JSON de toute structure : sélecteur de fichier natif (tauri-plugin-dialog), lecture en flux sans jamais garder le fichier en mémoire, profils de correspondance enregistrés dans le dossier de configuration et exportables, suggestions d'après les 20 premières entrées, entrées partiellement reconnues conservées, résumé avant l'import. |
