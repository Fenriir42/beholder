# 0014: Server library

## Context

Slice 3c adds the server library (SPEC §5.2): one shared set of entries per server, for homebrew and content from books the group owns, edited in the app and imported or exported as JSON and YAML. It has to sync like everything else (offline-first, commands through the outbox, deduplicated), work with the compendium's search and name priority, let a campaign hide entries, and stay cheap on a slow link even when the library grows (bulk imports come in slice 3e).

## Decision

- **Model:** `LibraryEntry { id, author, name, facets, text, book, rules, version }` in `beholder-core`, reusing the validated title and body types and the compendium `Facets` (bounded lengths). Commands: `Save { draft, base_version }` (0 creates) and `Delete`. Events: `Saved`, `Deleted`, append-only.
- **Rights** (pure function with matrix cases): anyone on the server adds; the author or anyone who is DM of at least one campaign edits or deletes.
- **Edit conflicts:** a save based on an older version is refused with `Stale`, and the UI asks to reload. Unlike notes, no conflict copies: entries are reference material, rarely edited at the same time.
- **Server storage:** a `library_log` with its own sequence and a `library_entries` projection with tombstones. Library commands share the global dedupe table, so resends are safe.
- **Sync over HTTP, notified over the WebSocket:**
  - Every change broadcasts a tiny `LibraryChanged { seq }` to all connections.
  - The client's compendium task then fetches `GET /api/library/changes/{after}`: the changes since its position, or `None` beyond a 1,000-change window or when the client is ahead.
  - In that case, it downloads the library bundle listed in the compendium index. The bundle is rebuilt only when the library moved, compressed at zstd level 3, and cached in RAM.
  - Only that one task writes library data, so a live edit can't race a bundle download.
- **Compendium integration:** the library is source `library` with priority 0, so it wins name clashes. Entries carry their book, rules version and library metadata (author, version), which made bundle format 3. Library changes are applied to the local index in place, keeping FTS5 in step.
- **Per campaign:** the library is a rules source, on by default for new campaigns. `Campaign.hidden` lists hidden entries, changed by a DM-only `HideEntry`. Players' searches exclude them; the DM sees them marked. This field required protocol v4 and a client cache reset.
- **Files:** the documented `beholder-library` format, version 1, in JSON (`serde_json`, at Tauri's version) and YAML (`serde_norway` 0.9.42). It's parsed in `client-core`. An import skips duplicates (same kind, name, book and rules version), reports invalid entries, and sends the rest as ordinary commands. Export writes to the Downloads folder; there's no save dialog, which would have added `tauri-plugin-dialog` and `tauri-plugin-fs`.

## Alternatives

- **Library events over the WebSocket:** they would race with bundle downloads on the client and need buffering. The notice-plus-HTTP design keeps a single writer.
- **Refetching the whole bundle on every change:** fine for a small library, wasteful once imports add thousands of entries.
- **Conflict copies like notes:** more machinery for a rare case on non-collaborative content.
- **`serde_yaml`:** deprecated. `serde_yml`: unclear maintenance. `serde-saphyr`: pure Rust, but writing YAML wasn't clearly supported.

## Consequences

- The desktop binary grew by 218 KB and the shipped WASM by 112 KB.
- The library bundle is cached in server RAM. Large libraries (slice 3e) should move it to disk, as SPEC suggests.
- A client more than 1,000 changes behind downloads the whole library once.
- Exported files land in Downloads under a numbered name; a save dialog can come later.
