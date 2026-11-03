-- Encounters of each campaign, with their combatants and map (body is a postcard Encounter).
CREATE TABLE encounters (
    id          BLOB PRIMARY KEY,
    campaign_id BLOB NOT NULL REFERENCES campaigns(id) ON DELETE CASCADE,
    body        BLOB NOT NULL
);
CREATE INDEX encounters_campaign ON encounters(campaign_id);
