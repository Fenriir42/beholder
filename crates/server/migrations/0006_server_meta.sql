-- Server-wide markers, such as whether the setup import from an external repository has run.
CREATE TABLE server_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
