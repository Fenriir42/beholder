# 0002: rusqlite on the server instead of sqlx

## Context

SPEC §2 named `sqlx` for the server and `rusqlite` for the client cache. The developer's laptop is slow, compile time is a priority, and the server serves one friend group.

## Decision

The server uses `rusqlite` (bundled SQLite) like client-core. One connection behind a `Mutex`, used through `Db::call`, which runs closures on tokio's blocking pool. Migrations are plain `.sql` files applied in order, tracked by `PRAGMA user_version`. Pragmas: WAL, `synchronous=NORMAL`, a 4 MiB page cache, no mmap.

Serializing every database call is also what makes the sync design simple: a command's commit and its fan-out happen inside the same call, so frames enter each connection's queue in commit order, and a connection's catch-up can't interleave with a commit (ADR-0004).

## Alternatives

- **sqlx 0.9:** async API and compile-time checked queries, but needs Rust 1.94, adds sqlx-core, sqlx-sqlite and proc-macros, and builds the same `libsqlite3-sys`. Checked queries also need a database or an offline cache at build time.
- **A reader pool next to the writer:** more concurrency than this load needs. If reads ever become a bottleneck, add 2 read-only connections behind `Db::call_read`.

## Consequences

- One SQLite build for the whole workspace; much faster clean builds.
- SQL is checked by tests against an in-memory database, not by the compiler.
- All database work is serialized; at this scale it's microseconds per call.
