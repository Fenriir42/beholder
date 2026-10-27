---
title: Sync and the protocol
description: How clients and the server stay consistent over unreliable links.
sidebar:
  order: 3
---

The design is recorded in ADR-0003 (wire protocol), ADR-0004 (sync) and ADR-0007 (client networking).

## Transport and frames

- One WebSocket over TLS (`wss://host/ws`) for live sync; plain HTTPS for auth and images.
- Every frame is `[flags: u8][postcard body]`. Bit 0 means the body is zstd-compressed, which happens only above 1 KB and when it actually saves bytes. Frames are capped at 4 MiB, compressed or not.
- HTTP bodies are postcard too, so there's no JSON on the wire.
- An idle connection sends only a ping every 15 s (a few hundred bytes per minute). No traffic for 35 s means the connection is dead.

## Handshake and catch-up

```
client                                   server
  Hello { version, token, known: [(campaign, last_seq)] } ─▶
                    ◀─ HelloOk { user, campaigns, rotated_token? }
                    ◀─ per campaign: Events (missed ones) or Snapshot
                    ◀─ per campaign: Presence
  (outbox resent in order) ─▶
```

A snapshot replaces events when the client knows nothing about the campaign, is behind the compaction point, is more than 1,000 events behind, claims a seq the server doesn't have, or when the missed range changes visibility (a note's audience or the campaign settings), because replay is filtered with today's audiences.

## Event log

Each campaign has an append-only log with a monotonically increasing `seq`. A command runs in **one SQLite transaction**: dedupe check, permission check, event append and projection update (the `campaigns`, `memberships` and `notes` tables). `Campaign::apply` in `beholder-core` is the single state machine, used by the server's tests and by every client cache.

## Per-recipient filtering

`permissions.rs` decides, per user (`Viewer { user, role }`), which events and which parts of a snapshot each member receives. Example: `InviteRegenerated` goes to the DM only, and a note goes only to its audience, so others see a gap in `seq`, which is normal. When a note's audience changes, users who gain access receive the full note and users who lose it receive a removal. A settings change re-sends every member a filtered snapshot. Fan-out encodes at most once per user. See ADR-0011.

## Commands, dedupe and the outbox

- The client writes each command to its **outbox** (SQLite) *before* sending it, with a UUIDv7 id.
- The server stores the reply (`Ack` or `Reject`) for every command id for 8 days. A resent command gets the stored reply and changes nothing.
- A command whose UUIDv7 timestamp is older than 7 days is rejected as `Expired`, because the server can no longer prove it hasn't seen it.
- The UI shows **confirmed state + pending commands**. A `Reject` removes the command and the view is recomputed. That's the whole rollback.

## Compendium bundles

Rules content doesn't travel over the WebSocket. Each source is a prebuilt, content-hashed bundle (postcard + zstd, ADR-0012). After connecting, the client fetches `GET /api/compendium` (the list of sources and their SHA-256), downloads any bundle it doesn't have from `GET /api/compendium/{sha256}` (immutable, resumable with `Range`), verifies the hash, and indexes the entries in its local SQLite with FTS5. Search never touches the network.

## Server library

The library has its own log and sequence, outside any campaign. Library commands go through the normal outbox and dedupe. Each change broadcasts `LibraryChanged { seq }` to every connection; the client's compendium task then calls `GET /api/library/changes/{after}`, which returns the changes since that position or, beyond a 1,000-change window, nothing, in which case the client downloads the library bundle from the compendium index. Only that task writes library data on the client. See ADR-0014.

## Ordering guarantees

All database work goes through one connection (`Db::call`). A command's commit and its fan-out happen in the same call, and so do a new connection's catch-up and subscription. So a client can't miss an event or receive one out of order.

## Bounded memory

Each connection has a 256-frame outgoing queue. A client too slow to drain it is disconnected and catches up on reconnect, instead of growing server memory.

## Connection states (client)

`Connecting` → `Online` → on loss `Reconnecting { queued }` → after 3 failures `Offline { queued }`. Retries back off from 0.5 s up to 10 s with jitter. `network_changed()` retries immediately.

## Tests that guard all this

- `crates/protocol`: every message round-trips; bad frames and zip bombs are rejected.
- `crates/server/src/permissions.rs`: the permission matrix.
- `crates/server/tests/ws.rs`: over real sockets, a player never receives the invite code, live or on catch-up; resent commands apply once.
- `crates/e2e/tests/flaky_network.rs`: a proxy adds latency and kills connections at random; no command is lost or duplicated, and both clients converge.
