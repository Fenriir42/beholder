# 0011: Notes, per-user audiences and conflict copies

## Context

Phase 2 (SPEC §5.3) adds Markdown notes and handouts. Each note has an audience: `private`, `party` or specific members. The DM can read players' private notes only when `dm_can_read_private_notes` is on, and **Reveal** changes an audience in one action. Until now the server filtered by **role** only, and shared one encoded frame per role (ADR-0004). Notes need filtering per **user**.

## Decision

- **Model** (`beholder-core`): `Note { id, author, title, body, audience, version, conflict_of }` lives in `Campaign.notes`, sorted by id, so client and server converge whatever order upserts arrive in. Four events are appended: `NoteCreated`, `NoteEdited`, `NoteAudienceChanged { audience, previous }`, `NoteDeleted`. Bodies are capped at 64 KB.
- **Commands**: `CommandKind::Note { campaign_id, op: NoteOp }`. `NoteOp::predicted_event` is shared by the client's optimistic view and the server.
- **Rules** (`permissions.rs`):
  - read: author, members named in the audience, or the DM for `Private` notes when the setting is on
  - edit: author or audience members (the DM's read access through the setting is read-only)
  - change audience or delete: author only
  - a note the actor can't read is answered with `NotFound`, never `Forbidden`, so its existence doesn't leak
- **Fan-out**: `Viewer { user, role }` replaces `Role`, and the hub shares frames per user. `NoteAudienceChanged` is translated per recipient: users who gain access get `NoteCreated` with the full current note, users who lose it get `NoteDeleted`, others get nothing. A settings change sends every subscriber a fresh filtered snapshot.
- **Catch-up**: replayed events are filtered with today's audiences and settings. That's only correct when neither changed inside the replay range, so such a range is answered with a snapshot instead.
- **Projection**: a `notes` table; deleted notes stay as tombstones (body cleared) so fan-out can still tell who saw them.
- **Conflicts**: `NoteEdited` carries the version it was based on. A stale edit doesn't fail: the server creates a conflict copy (id = the command's UUIDv7, private to the editor, `conflict_of` = the original), and the UI links the two.
- **Protocol**: `Campaign` gained a field and travels in `Snapshot`, so `PROTOCOL_VERSION` is now 2. The client cache schema is bumped too; caches rebuild from the server.
- **Rendering**: `pulldown-cmark` (0.13.5, `default-features = false, features = ["html"]`). Raw HTML renders as text, images load only as `img:<hash>` from our server, and links are inert spans that show their URL. The CSP backs this up.

## Alternatives

- **Separate note messages outside `Campaign`**: would avoid the protocol bump, but adds a second sync path and cache table for the same lifecycle.
- **Per-event before/after state for replay**: exact without snapshots, but needs history the projection doesn't keep. Audience and settings changes are rare, so a snapshot is cheaper.
- **Rejecting stale edits**: simpler, but the user loses typing. A conflict copy keeps both texts, as SPEC §3 asks.
- **Hand-written Markdown subset**: smaller, but another parser to maintain and test.
- **Clickable links**: the webview would navigate away from the app. An opener plugin can be added later.

## Consequences

- One frame is encoded per user instead of per role. With a handful of members this costs nothing measurable.
- A snapshot carries every note its recipient can see, and frames are capped at 4 MiB. Typical notes allow several hundred per campaign; splitting snapshots is deferred until it's needed.
- Edits are whole-document replaces debounced to 1 s of idle time, so each pause in typing stores the full body in the event log until compaction.
- Old clients get "please update" from the new server.
