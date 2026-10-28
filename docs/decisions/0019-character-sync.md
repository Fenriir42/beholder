# 0019: Character sheets: model, sync and visibility

## Context

Phase 4 (SPEC §5.1) adds assisted character sheets. Each player can have several characters per campaign, with one active. The DM, and only the DM, edits other players' characters. Other players see a summary only: name, species, classes and levels, AC, HP and the active flag. Sheets change often during play (HP, slots, conditions), sometimes from two people at once, such as the DM applying damage while the player edits their inventory, and often over unstable connections.

## Decision

- **Rules in `core`:** `rules.rs` holds the pure calculations: modifiers, proficiency bonus, saves and skills (with expertise and Jack of All Trades), AC formulas, spell slots, and the 2024 exhaustion penalty to d20 tests. Spell slots come from a caster type per class (full, half, half rounded up, third, pact), using the standard table. A single caster uses its own progression (2014 half casters start at level 2, 2024 ones at level 1, third casters at level 3). Multiclassing adds full levels, half levels (rounded down in 2014 and up in 2024), and a third of third-caster levels; pact magic is separate. `character.rs` holds the `Character` model and computes everything from it. Unit tests cover levels 1 to 20.
- **Targeted commands, full-state events:** `CommandKind::Character { campaign_id, op }` carries `CharacterOp::Create`, `SetActive`, `Delete`, or `Change { id, change }`. A change is one targeted edit: HP, a skill, the inventory list, the abilities, and so on. The server applies it to the latest stored character, so concurrent edits to different parts of a sheet all survive; edits to the same list are last-writer-wins. The resulting event, `CampaignEvent::CharacterSaved`, carries the whole character. `CharacterOp::events` builds these events and is shared by the server and the client's optimistic view.
- **Visibility on the server:** `permissions::can_edit_character` (the owner or the DM) decides both reading and editing the full sheet. Everyone else receives `PartyMemberSaved { summary }`, built from the full character at fan-out time, and `Campaign.party` instead of `Campaign.characters` in snapshots. `PartyMemberSaved` is never stored. Replayed events are filtered the same way, and no setting changes the rule, so catch-up needs no extra snapshot case.
- **Storage:** a `characters` table holds one postcard `Character` per row, with tombstones for deleted ones, and loads into the campaign snapshot.
- **Validation:** `Character::validate` caps names (80 characters), short texts (120), lists (200, or 500 for spells and inventory), notes (64 KB), abilities (1 to 30) and levels (1 to 20, total 20), so a sheet stays well under the 4 MB frame limit.
- **Protocol:** `Campaign` gained `characters` and `party`, so `PROTOCOL_VERSION` is now 5 and the client cache schema is now 4. Caches rebuild from the server.

## Alternatives

- **Saving the whole sheet with a version check:** simpler commands, but a stale version would reject the edit or produce conflict copies of a sheet, and a DM's HP change would routinely clash with the player's own edits.
- **Targeted events too:** smaller events, but filtering would need the full character at fan-out to build the summary anyway, and every client would need the same apply logic for 26 kinds of change.
- **Separate character messages outside `Campaign`:** they would avoid the protocol bump, but add a second sync path and cache for the same lifecycle.

## Consequences

- Each change sends the whole character to its owner and the DM, a few kilobytes compressed. That's acceptable for the frequency of play edits.
- A future rules engine or level-up assistant can build on `Character::apply` and the same events.
