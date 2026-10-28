# 0016: Undoable library imports

## Context

SPEC §5.2 asks that each import be recorded as a batch that can be undone in one action. An import of the `dnd-data` files writes tens of thousands of entries over many `UpsertMany` commands, and some of those entries replace entries that were already in the library. After the import, people may edit some of the imported entries.

## Decision

- **An import has an id:** the client makes an `ImportId` (UUIDv7) for each file import or paste and sends its batches as `LibraryOp::ImportMany { import, items }`. The variant is appended, so there's no protocol bump (ADR-0003). It follows the same per-item rules as `UpsertMany`. Only the user who started an import can add batches to it, and an undone import takes no more batches.
- **The server records what each import touched:** `library_imports` has one row per import (author, time, entry count, undone, kept). `library_import_items` holds, for each entry, the `library_log` sequence of its state before the import (NULL if the import created it) and the sequence of the import's own write.
- **Undo keeps later work:** `LibraryOp::UndoImport { import }` deletes the entries the import created and restores the previous text of the ones it replaced, read from the library log, with a new version. It leaves alone any entry whose sequence changed after the import, meaning it was edited or deleted since, and stores that count as `kept`. The importer, or anyone who is DM of a campaign, can undo an import (`permissions::can_undo_import`, in the permission matrix). It all happens in one transaction and one command.
- **Listing:** `GET /api/library/imports` returns the last 50 imports with author name, time, entry count and undo state. The interface shows an **Undo this import** button after each import and a **Recent imports** list.

## Alternatives

- **Revert everything, overwriting later edits:** simpler, but it silently throws away work done after the import. The user chose to keep later edits.
- **Undo on the client, by sending deletes and saves:** no server change, but it isn't atomic, can't see what the import replaced once the client cache moves on, and would send tens of thousands of commands.
- **Store a full copy of the replaced entries:** the library log already has every previous state, so the server only stores its sequence number.

## Consequences

- An undo writes one log event per entry it reverts. Undoing a 30,000-entry import makes the library bundle rebuild on the next sync, as any large change does.
- `UpsertMany` stays for compatibility, but current clients no longer send it.
- An import can't be undone twice, and an undo can't be undone. Re-importing the file does the same job.
