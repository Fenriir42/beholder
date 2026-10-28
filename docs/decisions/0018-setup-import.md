# 0018: Importing an external repository at server setup

## Context

SPEC §5.2 asks that, when the server is installed, the admin can import a repository structured like `nick-aschenbach/dnd-data` into the server library, through the same batched path as other imports and as one batch that can be undone. The server is a static musl binary on a `scratch` image, runs on a 1 GB VPS, and has no setup screen of its own.

## Decision

- **Configuration:** `BEHOLDER_IMPORT_REPO` (and `BEHOLDER_IMPORT_REF`, default `HEAD`) in the environment, which the docker-compose file passes through. There's no interactive prompt: a headless container has nowhere to show one.
- **Archive download, not git:** the server derives the `.tar.gz` archive URL (GitHub/Gitea/Codeberg `…/archive/<ref>.tar.gz`, GitLab `…/-/archive/<ref>/<name>-<ref>.tar.gz`, or a direct `.tar.gz` link) and downloads it with `ureq` over rustls, with ring and the bundled Mozilla roots, since the image has no system certificates. It reads the stream through `flate2` (pure-Rust backend) and `tar`, without saving the archive, and parses each matching JSON file one record at a time. No git binary or library is needed.
- **Shared converter:** the `dnd-data` converter moves from `xtask` into a new crate, `beholder-dnd-data`, so the server's import gets the same formatting, tables and subclass split as `cargo xtask dnd-data`. The crate adds `to_draft` (converter output to typed entry fields) and `each_record` (streams a top-level array).
- **One undoable import:** the entries go in batches of 500 through `library::decide` with `LibraryOp::ImportMany` and a fixed server actor (the nil user id), so they show under Recent imports and can be undone by any DM (ADR-0016). Each batch broadcasts a library change notice. Duplicates, by type, name, book and rules version, are skipped against the library and within the archive.
- **Runs once:** a `server_meta` row marks a successful import. A failed import is logged and tried again at the next start, and entries already written are skipped then.

## Alternatives

- **Shelling out to `git clone`:** it needs git in the image, which is `scratch`, and writes the whole repository to disk.
- **`gix`:** a pure-Rust git implementation, but much heavier to compile and link than tar and gzip.
- **Importing from the desktop app instead:** no server change, but it deviates from SPEC. The desktop's dnd-data converter and mapping profiles remain available as well.

## Consequences

- The server gains `ureq` (rustls, ring, webpki-roots), `tar`, `flate2` and `beholder-dnd-data`. The release binary is measured in CI.
- Entries imported this way have no human author, so only DMs can edit them.
- Memory stays bounded by one record plus a 500-entry batch, whatever the archive size. Single JSON files over 512 MB are skipped.
