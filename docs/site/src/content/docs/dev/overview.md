---
title: Architecture overview
description: Crates, boundaries and the rules every change must follow.
sidebar:
  order: 1
---

Beholder is one Rust workspace: a server, a desktop client (Tauri 2 + Leptos) and shared crates. `docs/SPEC.md` is the reference for the whole project; `CLAUDE.md` holds the working conventions.

```
 Leptos UI (wasm32)  ──invoke/events──▶  Tauri shell  ──▶  client-core  ══ wss + https ══▶  server
 beholder-ui                             beholder-desktop  beholder-client-core          beholder-server
        └──────────── beholder-ipc ───────────┘                 └──── beholder-protocol ────┘
                                         everything shares beholder-core
```

## Crates

| Path | Crate | Role |
|---|---|---|
| `crates/core` | `beholder-core` | Domain types, validation, campaign events and their pure `apply`. No IO, builds for wasm32. |
| `crates/protocol` | `beholder-protocol` | Wire messages, `PROTOCOL_VERSION`, HTTP bodies; postcard + zstd codec behind the `codec` feature. |
| `crates/server` | `beholder-server` | axum + SQLite: auth, campaigns, event log, sync, presence, images, permissions. |
| `crates/client-core` | `beholder-client-core` | Connection manager, outbox, SQLite cache, optimistic view, HTTPS client. No Tauri dependency. |
| `crates/e2e` | `beholder-e2e` | Test-only: the flaky-network test across server and client-core. |
| `apps/desktop/ipc` | `beholder-ipc` | Types crossing the Tauri boundary (UI ⇄ shell). |
| `apps/desktop/src-tauri` | `beholder-desktop` | Tauri shell: commands, events, keyring, `img://` image cache. |
| `apps/desktop/ui` | `beholder-ui` | Leptos CSR app: screens, Fluent EN/FR, design tokens. |

## Rules that are never bent

- **The server filters what each client receives.** Every permission rule lives in `crates/server/src/permissions.rs` and has a case in its permission-matrix test. Campaign data can only be sent as an `Outgoing`, which is only constructible through the filters.
- **The UI never touches the network.** UI → Tauri commands → client-core → server. Even images go through the shell's `img://` protocol.
- **Every change is a command** with a UUIDv7 id that is safe to resend; the server deduplicates.
- **Protocol changes are additive** (append enum variants) or bump `PROTOCOL_VERSION`. Every message has a round-trip test.
- **No CDNs or Google services at runtime**; fonts and icons are bundled.
- **Only SRD 5.2 content ships with the app.**

## Priorities

Optimisation first (CPU, RAM, disk, network, binary and WASM size), then maintainability, then "Rust wherever reasonable". When two conflict, the tradeoff is named and written down as an ADR in `docs/decisions/`.
