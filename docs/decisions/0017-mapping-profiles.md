# 0017: Importing JSON of any shape with mapping profiles

## Context

SPEC §5.2 asks for bulk imports of JSON files in any layout, read through saved and exportable **mapping profiles**, with suggestions from the first entries, entries kept even when only partly mapped, and files of about 100 MB read without holding them in memory. Until now the app read only its own library format, and the file was sent whole to the shell as a raw IPC body.

## Decision

- **Native file picker:** `tauri-plugin-dialog` 2.8.4 (default `gtk3` feature, so no new system library on Linux) gives the shell a file path. The shell reads the file from disk itself; the interface never sees its contents. The desktop binary grew from 5,830,600 to 6,127,464 bytes (+5.1%), which also covers the new import code. `rfd` alone would avoid `tauri-plugin-fs`, but its blocking GTK dialog doesn't fit Tauri's main loop on Linux, so it was rejected.
- **Streaming reader:** `client-core::mapping` walks the JSON with a serde `DeserializeSeed`. `probe` finds the first array of objects, at most three levels deep, keeps the first 20 entries and stops reading. `each` streams the entries at a given path one value at a time. The import runs the reader on its own thread and feeds the existing batched `ImportMany` path through a bounded channel of 512 entries, so memory stays flat whatever the file size, and the import can be undone (ADR-0016).
- **Profiles:** `beholder_core::mapping::Profile` holds the path to the entries, the keys for name, text, type, book and the structured fields (dotted paths such as `properties.Level`), a table from the file's type values to Beholder kinds, a default kind, a fixed book and the rules version, optionally read from the book name. The types live in `core` because the WASM interface edits them; the logic lives in `client-core`. Profiles are saved in `mapping-profiles.json` in the app's config folder and can be exported to and imported from JSON files.
- **Suggestions:** key names are matched case-insensitively against short lists (`name`/`title`, `description`/`text`, `category`/`type`, `challenge rating`, `item rarity`…), and type values are matched to kinds, plurals included. On the user's `dnd-data` files the suggestion needs no edits.
- **Never rejected:** an entry missing a required field gets "Unknown" (or CR 0, size Medium) and counts as partly mapped. A magic item without a rarity becomes equipment. Unmapped scalar keys are appended to the text as a "**Key:** value" list. Only entries without a name are refused.
- **Summary review:** **Check the whole file** streams the file once, without writing, and shows counts per type and per book, duplicates, partly mapped entries and unreadable ones. Then the import runs, with skip or replace for all duplicates.

## Alternatives

- **Keep the HTML file input:** no new dependency, but the whole file would be held in memory and copied into the shell.
- **A JSONPath or jq-like language for profiles:** more powerful, but heavier to implement and to explain. Dotted paths cover the layouts seen so far.
- **Profiles on the server:** shareable across the group, but it needs new protocol messages and storage. Exported files can be shared instead.

## Consequences

- The `dnd-data` converter stays the better route for those files, because it restores formatting and splits subclasses. The generic importer keeps their text as it is.
- The library export could use the same plugin for a save dialog. It still writes to the Downloads folder for now.
