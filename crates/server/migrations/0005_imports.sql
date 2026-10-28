-- One row per library import, so it can be undone in one action.
CREATE TABLE library_imports (
    id      BLOB PRIMARY KEY,
    actor   BLOB NOT NULL,
    ts      INTEGER NOT NULL,
    entries INTEGER NOT NULL DEFAULT 0,
    undone  INTEGER NOT NULL DEFAULT 0,
    kept    INTEGER NOT NULL DEFAULT 0
);

-- prev_seq is the library_log seq of the entry before the import (NULL if the import created it);
-- seq is the import's own write, used to tell whether the entry was edited since.
CREATE TABLE library_import_items (
    import_id BLOB NOT NULL,
    entry_id  BLOB NOT NULL,
    prev_seq  INTEGER,
    seq       INTEGER NOT NULL,
    PRIMARY KEY (import_id, entry_id)
);
