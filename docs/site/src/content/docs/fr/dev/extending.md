---
title: Ajouter une fonctionnalité
description: La liste de contrôle pour ajouter une nouvelle action utilisateur de bout en bout.
sidebar:
  order: 4
---

Chaque action utilisateur suit le même chemin. Exemple : ajouter une action hypothétique « définir la description de la campagne ».

## 1. Domaine (`beholder-core`)

- Ajoutez le type validé si nécessaire (par exemple `CampaignDescription` avec `serde(try_from)`, pour qu'une valeur invalide ne puisse même pas être désérialisée).
- Ajoutez l'opération à `CampaignOp` et l'événement à `CampaignEvent`, **à la fin** de chaque énumération.
- Gérez l'événement dans `Campaign::apply` et la prédiction dans `CampaignOp::predicted_event`.
- Testez les deux.

## 2. Protocole (`beholder-protocol`)

- Si le changement ne fait qu'ajouter des variantes, il est compatible. Sinon, faites monter `PROTOCOL_VERSION`.
- Ajoutez un exemple de chaque nouvelle variante aux tests aller-retour. Le `match` exhaustif de `codec.rs` ne compilera pas tant que ce n'est pas fait.

## 3. Permissions (`beholder-server/src/permissions.rs`)

- Décidez qui peut appliquer l'opération (`can_apply`) et qui peut voir l'événement (`event_for`, `campaign_for`).
- Ajoutez des lignes à la matrice des permissions. **Aucune règle de permission ne vit ailleurs.**

## 4. Serveur

- Gérez l'opération dans `campaigns::execute` et l'événement dans `campaigns::project` (la projection SQL doit rester égale au repli des événements ; le test `projection_equals_event_fold` le vérifie).
- Ajoutez une migration si le schéma change (`crates/server/migrations/000N_*.sql`).
- Si l'événement porte des données cachées, complétez `crates/server/tests/ws.rs` pour prouver qu'elles n'atteignent jamais le mauvais client.

## 5. Client et UI

- client-core n'a besoin de rien de nouveau : commandes, file d'envoi et annulation sont génériques.
- Ajoutez une commande Tauri dans `apps/desktop/src-tauri/src/commands.rs`, qui valide l'entrée et appelle `submit`.
- Ajoutez le contrôle d'UI et **chaque texte dans les deux fichiers**, `locales/en/main.ftl` et `locales/fr/main.ftl`. Utilisez les design tokens de `style/tokens.css`, jamais de couleurs en dur.

## 6. Décider, consigner, committer

- Nouvelle crate, changement de protocole, de synchro ou de stockage, ou écart avec la SPEC : écrivez un court ADR dans `docs/decisions/`.
- Justifiez toute nouvelle dépendance (pourquoi, son coût, l'alternative plus légère écartée) et indiquez la variation de taille du WASM ou du binaire pour l'UI ou le bureau.
- De petits commits Conventional Commits (`feat(server): …`) qui compilent et passent les tests.
