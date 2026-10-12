-- Phase 1 schema. Ids are 16-byte UUIDs, times are Unix milliseconds.

CREATE TABLE users (
    id           BLOB PRIMARY KEY,
    username     TEXT NOT NULL UNIQUE COLLATE NOCASE,
    display_name TEXT NOT NULL,
    created_at   INTEGER NOT NULL
);

CREATE TABLE password_credentials (
    user_id    BLOB PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    phc        TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Discord (and later providers) link here; empty in Phase 1.
CREATE TABLE external_identities (
    provider   TEXT NOT NULL,
    subject    TEXT NOT NULL,
    user_id    BLOB NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (provider, subject)
) WITHOUT ROWID;

-- Only token hashes are stored. `replaced_by` points at the rotated token;
-- this row stays valid until the new one is first used.
CREATE TABLE sessions (
    token_hash   BLOB PRIMARY KEY,
    user_id      BLOB NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at   INTEGER NOT NULL,
    last_used_at INTEGER NOT NULL,
    expires_at   INTEGER NOT NULL,
    replaced_by  BLOB
) WITHOUT ROWID;
CREATE INDEX sessions_user ON sessions(user_id);
CREATE INDEX sessions_replaced_by ON sessions(replaced_by);

-- Projection of each campaign's event log, updated in the same transaction
-- as the event append.
CREATE TABLE campaigns (
    id                        BLOB PRIMARY KEY,
    title                     TEXT NOT NULL,
    cover_hash                BLOB,
    invite_code               TEXT UNIQUE,
    dm_can_read_private_notes INTEGER NOT NULL DEFAULT 0,
    combat_visibility         INTEGER NOT NULL DEFAULT 0,
    head_seq                  INTEGER NOT NULL DEFAULT 0,
    compacted_seq             INTEGER NOT NULL DEFAULT 0,
    created_at                INTEGER NOT NULL
);

CREATE TABLE memberships (
    campaign_id  BLOB NOT NULL REFERENCES campaigns(id) ON DELETE CASCADE,
    user_id      BLOB NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role         INTEGER NOT NULL, -- 0 = DM, 1 = player
    display_name TEXT NOT NULL,    -- as of joining, so the projection equals the event fold
    joined_seq   INTEGER NOT NULL, -- seq of the event that added the member; gives fold order
    PRIMARY KEY (campaign_id, user_id)
) WITHOUT ROWID;
CREATE INDEX memberships_user ON memberships(user_id);
CREATE UNIQUE INDEX memberships_one_dm ON memberships(campaign_id) WHERE role = 0;

-- Body is a postcard-encoded CampaignEvent (append-only variants, ADR-0003).
CREATE TABLE events (
    campaign_id BLOB NOT NULL REFERENCES campaigns(id) ON DELETE CASCADE,
    seq         INTEGER NOT NULL,
    ts          INTEGER NOT NULL,
    actor       BLOB NOT NULL,
    command_id  BLOB NOT NULL,
    body        BLOB NOT NULL,
    PRIMARY KEY (campaign_id, seq)
) WITHOUT ROWID;

-- Deduplication: the stored answer is replayed for a resent command id.
CREATE TABLE commands (
    id         BLOB PRIMARY KEY,
    user_id    BLOB NOT NULL,
    outcome    BLOB NOT NULL, -- postcard Reply (Ack or Reject)
    created_at INTEGER NOT NULL
) WITHOUT ROWID;
CREATE INDEX commands_created ON commands(created_at);

-- Encoded files live on disk under images/<first byte hex>/<hash>-{thumb,display}.webp.
CREATE TABLE images (
    hash          BLOB PRIMARY KEY,
    width         INTEGER NOT NULL,
    height        INTEGER NOT NULL,
    thumb_bytes   INTEGER NOT NULL,
    display_bytes INTEGER NOT NULL,
    uploader      BLOB NOT NULL,
    created_at    INTEGER NOT NULL
) WITHOUT ROWID;
