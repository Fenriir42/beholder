-- Projection of character events. Body is a postcard Character; deleted rows stay as tombstones.
CREATE TABLE characters (
    id          BLOB PRIMARY KEY,
    campaign_id BLOB NOT NULL REFERENCES campaigns(id) ON DELETE CASCADE,
    owner       BLOB NOT NULL,
    body        BLOB NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX characters_campaign ON characters(campaign_id, id);
