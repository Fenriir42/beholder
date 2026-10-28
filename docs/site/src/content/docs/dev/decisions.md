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
| 0011 | Notes: per-user filtering and frame sharing, reveal as upsert/removal, snapshot catch-up across visibility changes, conflict copies, protocol v2, sanitized Markdown with pulldown-cmark. |
| 0012 | Compendium: SRD vendored from 5e-srd-api (5e-database is archived), dev-only `beholder-xtask` converter, committed reproducible bundle, served immutable with Range, FTS5 search on the client. |
| 0013 | Open5e: per-book bundles converted from a pinned commit (raw data not vendored), generated OGL notices, per-campaign sources (SRD included), game-system tag, name-clash priority with 'Also in', protocol v3. |
| 0014 | Server library: author or any DM edits, stale saves refused, notice over WebSocket and changes over HTTP, library as top-priority source, per-campaign hidden entries (protocol v4), JSON/YAML files with serde_norway. |
| 0015 | Large library imports: SaveMany batches of 500, library bundle streamed to disk and rebuilt only when over 1,000 changes behind, raw-body file import, dnd-data converter, versioned file format with a JSON Schema. |
| 0016 | Undoable library imports: an ImportId per import sent with ImportMany batches, the server records each entry's previous log position, UndoImport deletes created entries and restores replaced ones but keeps entries edited since, by the importer or any DM. |
| 0017 | Importing JSON of any shape: native file picker (tauri-plugin-dialog), streaming reader that never holds the file, mapping profiles stored in the config folder and exportable, suggestions from the first 20 entries, partly mapped entries kept, summary review before importing. |
| 0018 | Setup import from an external repository: BEHOLDER_IMPORT_REPO, archive downloaded over rustls and streamed through tar and gzip, the dnd-data converter moved into the beholder-dnd-data crate, one undoable import by a server actor, run once. |
| 0019 | Character sheets: rules as pure functions in core, targeted change commands with full-state CharacterSaved events, full sheets for owner and DM and summaries for others filtered on the server, PROTOCOL_VERSION 5. |
| 0020 | Computed spell and weapon effects: core::actions parses spell and weapon texts into structured info (checks, damage, scaling), computes effects for the character as Roll values the dice system will roll, parsing done in the shell. |
