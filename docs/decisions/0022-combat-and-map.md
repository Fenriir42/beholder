# 0022: Combat tracker and battle map

## Context

SPEC Phase 6 asks for initiative, turns, HP and conditions with exact, vague or hidden monster health for players, and a basic square-grid battle map. The DM controls every shape, and each player moves only their own character's token. The user chose these options:

- prepare several encounters and run one;
- under the hidden rule, players get the map but not the tracker;
- the DM picks per encounter whether moves show while dragging or only on drop;
- players can move at any time unless the DM locks the map.

## Decision

- **`core::combat`:** an `Encounter` holds its combatants (with a source of character, compendium monster or custom), round and turn, a grid size (30×20 by default, at most 200×200), shapes (token, wall or zone, with position, size, label, colour, hidden flag, optional linked combatant and owner), a lock and a live-drag flag. `EncounterOp::events` validates an op and returns the events, so the server and client-core prediction share one code path.
  - Turns skip combatants without initiative. A condition's rounds count down at the end of its creature's turn.
  - Activating one encounter deactivates the others.
  - A linked token and its combatant share the hidden flag.
- **Events:** `EncounterSaved` carries the whole encounter and `EncounterDeleted` removes it. `ShapeMoved` is targeted, so a token move doesn't resend the encounter. Encounters live in their own table (migration 0009) and in the campaign snapshot. `PROTOCOL_VERSION` is 8 and the client cache schema is 7.
- **Permissions on the server:**
  - `combat::decide` lets players act only on the active encounter, and only with `MoveShape` on a token they own (when the map isn't locked) or `SetInitiative` for their own character's combatant.
  - `permissions::encounter_for` filters what each player receives. They get only the active encounter, never hidden shapes or combatants. Under `vague`, monsters carry a health band instead of HP and AC. Under `hidden`, they get no combatants and no turn state.
  - Deactivating an encounter reaches players as a deletion.
  - Every rule has cases in the permission matrix.
- **Live drag:** `ClientMsg::Preview` and `ServerMsg::Preview` carry a shape position while dragging. The server relays a preview only when the encounter allows live drag and the sender may move that shape. It never stores or acks a preview, so losing one costs nothing. The client sends at most one preview per cell change every 80 ms, a final one on drop, and then the `MoveShape` command. A received preview is drawn only while the shape is still where it was when the preview arrived.
- **Interface:** the Combat screen pairs the tracker with an SVG map. The map draws the grid as a single path, scales with a zoom setting, and uses pointer events with pointer capture for drags.
  - Initiative rolls go through the server dice path (ADR-0021). Once the roll comes back, the client sends `SetInitiative`.
  - Monster HP, AC and initiative are read from the compendium stat block by `actions::parse_monster`.
  - Player colours and the shape palette are design tokens. Tokens show initials so colour isn't the only cue.

## Alternatives

- **One event per field change:** smaller events, but many more variants to keep compatible. Encounters are small, except for moves, which have their own event.
- **Ephemeral moves as regular commands:** every drag step would go into the event log and the outbox. Previews are lossy on purpose.
- **A canvas map:** faster with thousands of shapes, but needs hit-testing and accessibility by hand. SVG handles the 500-shape limit and keeps shapes in the DOM.
- **Initiative rolled on the client:** instant, but not in the shared log and not authoritative.

## Consequences

- A character's combatant copies the sheet's HP when added. Damage in the tracker doesn't change the sheet.
- Monsters from converted `dnd-data` files have no stat block numbers, so they start with 1 HP, no AC and no initiative bonus until the DM edits them.
- Initiative rolled while offline is applied once the roll comes back, as long as the Combat screen stays open.
