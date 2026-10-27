# 0012: Compendium bundles and the xtask converter

## Context

Phase 3 starts with the SRD 5.2 compendium (SPEC §5.2). SPEC names `5e-bits/5e-database` as the source, but that repository was archived on 2026-10-02; its data now lives in `5e-bits/5e-srd-api` under `packages/5e-database`. The data is 25 JSON files (3.1 MB) in the shape of the 5e API, with cross-references, nested option sets and text that is close to Markdown. Clients must get it once, compressed, and search it offline (SPEC §3).

## Decision

- **Source:** vendor `packages/5e-database/src/2024/en/` from `5e-bits/5e-srd-api` at a pinned commit into `data/srd-5.2/upstream/`, verbatim, with its MIT licence and the CC-BY attribution. This replaces the archived repository named in SPEC.
- **Model** (`beholder-core::compendium`): an `Entry { key, name, facets, text }`. `key` is `kind/index` and unique within a source. `Facets` is an enum per kind holding only what the UI filters on (spell level, school and classes; monster CR, type and size; item rarity, category and attunement; …), so the kind can't disagree with its fields. `text` is the full entry as Markdown. A `Bundle` is `{ format, source: SourceMeta, entries }`; `SourceMeta` carries the id, name, rules version, upstream commit and attribution.
- **Converter:** a dev-only crate, `beholder-xtask` (`cargo xtask srd`). It reads the JSON as `serde_json::Value`, renders each kind to Markdown (spell header and table, full monster stat block with spell lists, class level table with spell slots, features grouped by level, species traits in book order), validates unique keys, non-empty text and the expected count per kind, and writes `data/srd-5.2/srd.bundle`: postcard, zstd level 19, entries sorted by key.
- **The bundle is committed.** The server embeds it, and neither the server build nor CI runs the converter. CI runs `cargo xtask srd --check`, which rebuilds the bundle and fails if it differs from the committed file. The bundle is byte-reproducible (tested).
- `serde_json` is reused at the version Tauri already locks, so the workspace gains no new crate.

## Alternatives

- **Typed structs for all 25 upstream schemas:** more code than the converter itself, for a one-way conversion that validation already guards.
- **Converting at server start or in the Docker build:** the server would carry `serde_json` and the raw data, or the image build would run Rust tooling on every release.
- **Shipping upstream JSON to clients:** about 15 times larger, and every client would need the converter.
- **Keeping the archived repository:** it lacks fixes already made upstream (classes, magic items, monsters).

## Consequences

- The SRD bundle is 200 KB (3.1 MB of JSON), downloaded once per version.
- Updating the SRD is: bump the commit in `data/srd-5.2/README.md` and `srd.rs`, re-copy `upstream/`, run `cargo xtask srd`, review with `cargo xtask srd-show`, and adjust the expected counts if upstream added entries.
- Mechanical data for character sheets (class progression, spell slots) stays in `upstream/Levels` and joins the model in Phase 4. Rules glossary entries (weapon properties, masteries, poisons, skills) aren't converted yet.
