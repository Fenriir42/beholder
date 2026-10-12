---
title: Design decisions
description: Summary of the ADRs in docs/decisions.
sidebar:
  order: 5
---

The full records are in `docs/decisions/`. Each one covers context, decision, alternatives and consequences.

| ADR | Decision |
|---|---|
| 0001 | Workspace layout, `beholder-*` names, pinned toolchain; `beholder-ipc` and `beholder-e2e` added; the local `data/` folder is never committed. |
| 0002 | `rusqlite` on the server too, instead of sqlx: one SQLite build, faster compiles, one serialized connection. |
| 0003 | Wire format: flag byte + postcard, zstd above 1 KB, 4 MiB cap, version in `Hello`, append-only evolution. |
| 0004 | Sync: per-campaign event log, per-recipient filtering through `Outgoing`, dedupe by UUIDv7 for 8 days, snapshot fallback, optimistic view = confirmed + pending. |
| 0005 | Auth: Argon2id (19 MiB), hashed opaque tokens, rotation with a grace period, rate limits, open registration. |
| 0006 | Images: header checks before decoding, capped decode, lossy WebP thumbnail + display size, content-addressed. |
| 0007 | Client networking: system TLS (native-tls), `ureq` for HTTPS, connection states and backoff. |
| 0008 | Desktop: no router, Tauri IPC without wrappers, raw-body uploads, `img://` image cache, Fluent, keyring. |
| 0009 | Deployment: static musl image on `scratch`, compose memory limits, Caddy HTTP/1+2, CI-built installers. |
| 0010 | Documentation with Astro Starlight, local only, on a user-local Node. |
