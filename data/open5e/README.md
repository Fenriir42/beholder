# Open5e sources

Each `*.bundle` here is one document from [open5e/open5e-api](https://github.com/open5e/open5e-api) (`data/v2/`) at commit `e69ac13b49dae6a9d84abf38fc674fe1db5b392b`, converted by `cargo xtask open5e`. The raw data isn't vendored: the converter fetches that commit into `target/open5e/` (set `OPEN5E_DIR` to use an existing checkout). Never edit the bundles by hand; `cargo xtask open5e --check` verifies them.

Skipped: `srd-2024` (the SRD 5.2 comes from `data/srd-5.2`), `tob` (replaced by `tob-2023`), `elderberry-inn-icons` (images only), and documents with no convertible entries.

## Licences

Every bundle carries the full text of its licences (OGL 1.0a, CC BY 4.0, CC0) and a notice shown on the Credits screen. The OGL 1.0a requires each work's Section 15 copyright notice. Open5e's data doesn't include them, so the notices are **generated** from each document's metadata (title, publication year, publisher, authors) and have **not** been checked against the books' own Section 15 text.
