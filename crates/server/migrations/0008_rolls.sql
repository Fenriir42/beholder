-- The last rolls of each campaign (body is a postcard RollRecord); older rows are pruned on insert.
CREATE TABLE rolls (
    campaign_id BLOB NOT NULL REFERENCES campaigns(id) ON DELETE CASCADE,
    seq         INTEGER NOT NULL,
    body        BLOB NOT NULL,
    PRIMARY KEY (campaign_id, seq)
);
