---
title: Adding a feature
description: The checklist for adding a new user action end to end.
sidebar:
  order: 4
---

Every user action follows the same path. Here, as an example, is adding a hypothetical "set campaign description" action.

## 1. Domain (`beholder-core`)

- Add the validated type if needed (e.g. `CampaignDescription` with `serde(try_from)`, so invalid values can't even be deserialized).
- Add the op to `CampaignOp` and the event to `CampaignEvent`, **at the end** of each enum.
- Handle the event in `Campaign::apply` and the prediction in `CampaignOp::predicted_event`.
- Unit-test both.

## 2. Protocol (`beholder-protocol`)

- If the change only appends enum variants, it's compatible. Otherwise, bump `PROTOCOL_VERSION`.
- Add a sample of every new variant to the round-trip tests. The exhaustive `match` in `codec.rs` won't compile until you do.

## 3. Permissions (`beholder-server/src/permissions.rs`)

- Decide who may apply the op (`can_apply`) and who may see the event (`event_for`, `campaign_for`).
- Add rows to the permission matrix tests. **No permission rule lives anywhere else.**

## 4. Server

- Handle the op in `campaigns::execute` and the event in `campaigns::project` (the SQL projection must stay equal to the event fold; `projection_equals_event_fold` checks it).
- Add a migration file if the schema changes (`crates/server/migrations/000N_*.sql`).
- If the event carries hidden data, extend `crates/server/tests/ws.rs` to prove it never reaches the wrong client.

## 5. Client and UI

- client-core needs nothing new: commands, outbox and rollback are generic.
- Add a Tauri command in `apps/desktop/src-tauri/src/commands.rs` that validates input and calls `submit`.
- Add the UI control and **every string to both** `locales/en/main.ftl` and `locales/fr/main.ftl`. Use design tokens from `style/tokens.css`, never raw colours.

## 6. Decide, write down, commit

- New crate, protocol, sync or storage change, or a deviation from SPEC: write a short ADR in `docs/decisions/`.
- Justify any new dependency (why, cost, the lighter alternative you rejected) and report the WASM or binary size change for the UI or desktop crates.
- Small Conventional Commits (`feat(server): …`) that each compile and pass tests.
