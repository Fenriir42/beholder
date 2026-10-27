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
