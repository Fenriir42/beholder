# 0004: Sync design (event log, filtering, outbox, dedupe, snapshots)

## Context

SPEC §3–4: the server is the authority, clients stay usable offline on a lossy VPN, hidden data must never be sent, and every user action is a command that's safe to resend.

## Decision

**Event log.** Each campaign has an append-only log with a monotonically increasing `seq`. A command runs in one transaction: dedupe check, validation, permission check, event append, and an SQL projection (`campaigns`, `memberships`) updated by the same `project` function for every event. The projection always equals the fold of the events through `Campaign::apply` (core), which both server and client use.

**Dedupe.** `commands(id, user_id, outcome)` stores the reply (`Ack` or `Reject`) for every command. A resent id gets the stored reply and produces nothing new. Rows are kept 8 days. A command whose UUIDv7 timestamp is older than 7 days is rejected as `Expired`: past the window the server can't prove it never applied it. An id reused by another user is `IdConflict`.

**Per-recipient filtering.** `permissions.rs` holds every rule as pure functions (`can_apply`, `event_for`, `campaign_for`) and the `Outgoing` type. `Events` and `Snapshot` messages can only be built through `Outgoing::events` / `Outgoing::snapshot`, which apply the filters; `Outgoing::control` refuses them. The hub only sends `Outgoing`. A filtered-out event leaves a gap in that client's seq, which is normal.

**Catch-up.** `Hello` carries `(campaign_id, last_seq)` for every cached campaign. The server answers `HelloOk` with the full membership list (the client drops anything else), then per campaign either the missed events or a snapshot. Snapshot when: the client has nothing, is behind the compaction point, is more than 1,000 events behind, or claims a seq ahead of the server.

**Ordering.** Commit + fan-out, and catch-up + subscribe, each run inside one `Db::call`. Since calls are serialized, a connection never misses an event or sees one out of order.

**Bounded memory.** Each connection has a 256-frame queue. A full queue drops the connection; the client reconnects and catches up. Encoded frames are shared between recipients with the same role.

**Presence.** In-memory connection counts per user. Online is announced on the first connection; offline 10 s after the last one closes, so VPN flaps don't spam presence.

**Compaction.** Daily: keep the last 1,000 events per campaign (moving `compacted_seq`), drop expired command replies and sessions.

**Client side (client-core).** The cache holds only *confirmed* state (server events and snapshots). The outbox holds pending commands in order. The UI view is confirmed state with pending commands applied on top via `CampaignOp::predicted_event`. `Ack` deletes the command from the outbox; `Reject` deletes it too, and the view recomputes, which is the rollback. Server events rebase pending commands automatically.

## Alternatives

- **Inverse operations for rollback:** more code per op and fragile when events interleave. Recomputing the view is trivial at this data size.
- **Per-client event cursors on the server:** unnecessary; the client's `last_seq` is enough.
- **CRDTs:** SPEC rules them out for v1.

## Consequences

Visibility changes in later phases (note reveals, `dm_can_read_private_notes`, combat visibility) extend `event_for` / `campaign_for` and may send upserts or removals per recipient; the delivery path doesn't change.
