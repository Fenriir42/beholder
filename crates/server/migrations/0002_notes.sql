-- Projection of note events. Deleted notes stay as tombstones (deleted = 1),
-- so fan-out can still decide who saw them.
CREATE TABLE notes (
    id          BLOB PRIMARY KEY,
    campaign_id BLOB NOT NULL REFERENCES campaigns(id) ON DELETE CASCADE,
    author      BLOB NOT NULL,
    title       TEXT NOT NULL,
    body        TEXT NOT NULL,
    audience    BLOB NOT NULL, -- postcard Audience
    version     INTEGER NOT NULL,
    conflict_of BLOB,
    deleted     INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX notes_campaign ON notes(campaign_id, id);
