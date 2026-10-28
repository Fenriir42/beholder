# 0015: Large library imports

## Context

The user's `dnd-data` files hold 33,983 entries (about 60 MB of JSON). The slice 3c design (ADR-0014) would have handled them badly:

- one command per entry, so 34,000 transactions and 34,000 notices to every connected client
- the whole file read into the interface's WASM memory, which never shrinks
- the full library bundle rebuilt in server RAM after every change, the next time any client synced

The files also use their own layout, which the importer refuses. SPEC §5.2 puts large imports in slice 3e; the user asked to make their files importable now.

## Decision

- **Batches:** `LibraryOp::SaveMany` creates up to 500 entries in one command, appended to the enum so there's no protocol bump. The server validates the whole batch before writing anything, because a rejected command still commits its stored reply. The client cuts imports into batches of 500 entries or about 1 MB, well under the 4 MiB frame limit, and the view shows a count of pending imported entries instead of their ids.
- **Library bundle on disk:** the server streams the bundle to `data_dir/library/` one entry at a time. It's byte-identical to `postcard(Bundle)`, which is tested. It's rebuilt only when missing or more than 1,000 changes behind the head. Clients download that bundle, then apply the change feed from its sequence number. Because the bundle's hash no longer changes with every edit, the client checks the library by position, not by hash.
- **Raw-body import:** the UI sends the picked file to the shell as a raw IPC body, like cover uploads, and the shell parses it on a blocking thread. The format travels in a header, because file names can contain characters that aren't valid in headers.
- **Converter, not a generic importer:** `cargo xtask dnd-data` converts the six `dnd-data` files into `beholder-library` files in the git-ignored `data/converted/`, restoring what structure it reliably can, and validates every entry with the app's own parser. The converted files never leave the user's machine.
- **The format is a standard:** a versioning policy (new optional fields within a version; readers ignore unknown fields; a newer version is refused) and a JSON Schema published with the docs. Both the converted files and deliberately broken samples are checked against the schema.

## Alternatives

- **Mapping profiles now (rest of 3e):** a general importer for any JSON shape. Much more UI, while only one known layout needed converting today.
- **Rebuilding the bundle in memory on every change:** simple, but up to about 150 MB of transient memory per rebuild on a 256 MB container, after every edit.
- **Bigger batches:** fewer commands, but frames close to the 4 MiB limit and longer server transactions.

## Consequences

- Importing all six converted files sends about 70 batch commands. Late clients download the library once, then follow the change feed.
- A server restart rebuilds the library bundle on the first index request, which takes a few seconds for 34,000 entries, done once.
- Converted descriptions stay partly flattened where no structure could be detected.
- The rest of slice 3e remains: mapping profiles for arbitrary JSON, undoable import batches, and the setup-time import from a git repository.

## Addendum: replacing on import

SPEC §5.2 asks that duplicates can be skipped or replaced, and re-importing an improved conversion was otherwise impossible. `LibraryOp::UpsertMany` (appended) carries creates (base version 0) and updates (the entry's current version) in one batch, with the same per-item rules as single saves (author or any DM; stale versions refused) and all-or-nothing validation. The client replaces only duplicates the user may edit and counts the rest as "not yours".
