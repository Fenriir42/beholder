# 0013: Open5e sources, per-campaign sources and name priority

## Context

Slice 3b adds the open-licensed books from Open5e (`open5e/open5e-api`, `data/v2/`): 25 documents, about 35 MB of Django fixtures in a schema different from the SRD's, mostly under the OGL 1.0a. SPEC §5.2 says each third-party source is toggled per campaign, the SRD is always on, and the data is vendored at a pinned commit. The user also asked for:

- every 5e-compatible source
- the DM choosing any source per campaign, the SRD included
- the group's own entries winning name clashes, with the other versions reachable
- room for other game systems later

## Decision

- **One bundle per document**, converted by `cargo xtask open5e` from a pinned commit. The converter loads every document into lookup tables first (spells name classes by pk in another document, subclasses point at their parent class, condition descriptions at the condition), then renders each kind to Markdown in the same layouts as the SRD.
- **Bundled:** every document with convertible content, except `srd-2024` (the SRD 5.2 comes from 5e-srd-api, ADR-0012), `tob` (replaced by `tob-2023`) and `elderberry-inn-icons` (images). That's 20 bundles, 1.59 MB.
- **Raw data isn't vendored** (a deviation from SPEC §5.2). The converter fetches the pinned commit into `target/open5e/` (or uses `OPEN5E_DIR`), and only the bundles are committed. CI runs `cargo xtask open5e --check`.
- **Licences:** every bundle carries the full texts of its licences, and Credits shows them once. OGL 1.0a requires each work's Section 15 notice, which the data lacks, so the notices are **generated** from document metadata (title, year, publisher, authors). At the user's choice they are **not verified** against the books. CC BY 4.0 sources get an attribution line.
- **Game systems:** `SourceMeta` carries a `GameSystem` (only `Dnd5e` now) and a `RulesVersion` (2014, 2024, Level Up). Level Up stays inside D&D 5e. Another system would be a new `GameSystem` variant, with campaigns using only sources of their own system.
- **Per-campaign sources:** `Campaign.sources` (default: SRD only), changed by a DM-only `SetSources` op (`SourcesChanged` event). The SRD can be turned off too (a deviation from SPEC §5.2). The server refuses unknown ids. Adding the field to `Campaign` bumps `PROTOCOL_VERSION` to 3 and the client cache schema to 3.
- **Downloads follow the campaigns:** a client downloads only the sources enabled in its campaigns, re-checks when campaign data arrives, and drops sources nothing uses any more.
- **Name clashes:** search folds entries with the same kind and name into one hit from the highest-priority source, counting the rest:
  1. the server library (slice 3c)
  2. the SRD 5.2
  3. other 2024 sources
  4. 2014 sources
  5. Level Up sources

  The entry view lists the other versions ("Also in").
- The server embeds every bundle through `build.rs` and reads each header by stream-decompressing only its start.

## Alternatives

- **Vendoring the raw data:** about 30 MB added to every clone, over a slow VPN, for files that are never read at runtime.
- **Converting Open5e at server start:** the server would carry the raw data, `serde_json` and the converter.
- **One bundle for all of Open5e:** simpler, but every client would download every book.
- **Keeping sources outside `Campaign`:** avoids the protocol bump, at the cost of a second sync path.
- **Verifying the OGL notices by hand:** the legally safer option, not chosen.

## Consequences

- The server binary grows by about 1.8 MB of embedded bundles, read from the page cache only when served.
- Updating Open5e: bump `COMMIT` in `open5e.rs` and `data/open5e/README.md`, run `cargo xtask open5e`, review with `cargo xtask show`.
- Generated OGL notices may be inaccurate. Replacing them with each book's real Section 15 text means a hand-written notices file read by the converter.
- Upgrading drops a client's queued offline changes once (cache schema 3).
