---
title: Vue d'ensemble de l'architecture
description: Les crates, leurs frontières et les règles que chaque changement doit respecter.
sidebar:
  order: 1
---

Beholder est un seul workspace Rust : un serveur, un client de bureau (Tauri 2 + Leptos) et des crates partagées. `docs/SPEC.md` est la référence du projet ; `CLAUDE.md` contient les conventions de travail.

```
 UI Leptos (wasm32)  ──invoke/events──▶  shell Tauri  ──▶  client-core  ══ wss + https ══▶  serveur
 beholder-ui                             beholder-desktop  beholder-client-core          beholder-server
        └──────────── beholder-ipc ───────────┘                 └──── beholder-protocol ────┘
                                       tout partage beholder-core
```

## Crates

| Chemin | Crate | Rôle |
|---|---|---|
| `crates/core` | `beholder-core` | Types du domaine, validation, événements de campagne et leur `apply` pur. Aucune E/S, compile pour wasm32. |
| `crates/protocol` | `beholder-protocol` | Messages réseau, `PROTOCOL_VERSION`, corps HTTP ; codec postcard + zstd derrière la feature `codec`. |
| `crates/server` | `beholder-server` | axum + SQLite : authentification, campagnes, journal d'événements, synchro, présence, images, permissions. |
| `crates/client-core` | `beholder-client-core` | Gestion de la connexion, file d'envoi (outbox), cache SQLite, vue optimiste, client HTTPS. Sans dépendance à Tauri. |
| `crates/e2e` | `beholder-e2e` | Tests uniquement : le test de réseau instable, serveur et client-core ensemble. |
| `apps/desktop/ipc` | `beholder-ipc` | Types qui traversent la frontière Tauri (UI ⇄ shell). |
| `apps/desktop/src-tauri` | `beholder-desktop` | Shell Tauri : commandes, événements, trousseau, cache d'images `img://`. |
| `apps/desktop/ui` | `beholder-ui` | Application Leptos CSR : écrans, Fluent FR/EN, design tokens. |

## Règles non négociables

- **Le serveur filtre ce que reçoit chaque client.** Toutes les règles de permission sont dans `crates/server/src/permissions.rs`, avec un cas dans la matrice de tests. Les données de campagne ne peuvent être envoyées que sous forme d'`Outgoing`, qui ne se construit qu'à travers les filtres.
- **L'UI ne touche jamais le réseau.** UI → commandes Tauri → client-core → serveur. Même les images passent par le protocole `img://` du shell.
- **Chaque modification est une commande** avec un identifiant UUIDv7 qu'on peut renvoyer sans risque ; le serveur déduplique.
- **Les changements de protocole sont additifs** (variantes ajoutées en fin d'énumération) ou font monter `PROTOCOL_VERSION`. Chaque message a un test aller-retour.
- **Aucun CDN ni service Google à l'exécution** ; polices et icônes sont embarquées.
- **Seul le contenu du SRD 5.2 est livré avec l'application.**

## Priorités

D'abord l'optimisation (CPU, RAM, disque, réseau, taille du binaire et du WASM), puis la maintenabilité, puis « du Rust partout où c'est raisonnable ». Quand deux priorités s'opposent, le compromis est nommé et consigné dans un ADR de `docs/decisions/`.
