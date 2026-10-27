-- Server-wide library. Body is a postcard LibraryEntry; deleted rows stay as tombstones.
CREATE TABLE library_entries (
    id      BLOB PRIMARY KEY,
    body    BLOB NOT NULL,
    deleted INTEGER NOT NULL DEFAULT 0,
    seq     INTEGER NOT NULL
);

-- Body is a postcard LibraryEvent (append-only variants, ADR-0003).
CREATE TABLE library_log (
    seq        INTEGER PRIMARY KEY,
    ts         INTEGER NOT NULL,
    actor      BLOB NOT NULL,
    command_id BLOB NOT NULL,
    body       BLOB NOT NULL
);

-- Postcard Vec<LibraryId> of library entries hidden in the campaign; NULL means none.
ALTER TABLE campaigns ADD COLUMN hidden BLOB;
