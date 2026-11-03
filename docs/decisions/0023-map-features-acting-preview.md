# 0023: Terrain, templates, acting panel and player preview

## Context

After the first combat slice (ADR-0022), the DM could do little on the combat page beyond running the tracker, and had no way to see the player side. The user asked for terrain, hazards, objects and area templates on the map, a way for the DM to play any monster or character, and a player preview. They chose all four map additions, the acting panel, and a preview that matches what the server really sends. SPEC §5.4 listed area templates as "later", so this moves them into scope.

## Decision

- **Shape model:** `ShapeKind` gains `Terrain(Terrain)`, `Hazard`, `Object(Object)` and `Template(Template { area, feet, angle })`, appended so older variants keep their encoding.
  - Every shape gains a `note` of up to 300 characters, readable by anyone who sees the shape.
  - `Shape::cells` returns the cells a shape covers. For templates, a cell counts when its centre is inside; cones use the 2024 width-equals-length rule, and lines are 5 ft wide. The scan is limited to the template's bounding box.
- **Wire and storage:** adding `note` changes the shape layout, so `PROTOCOL_VERSION` is 9 and the client cache schema is 8. The server decodes encounters stored before this change through a legacy struct, and `catch_up` already falls back to a snapshot for undecodable events.
- **Player templates:**
  - `permissions::can_put_shape` lets a player create or change a template they own: not hidden, not linked to a combatant, while the encounter is active and unlocked, up to `MAX_PLAYER_TEMPLATES` (10).
  - `can_remove_shape` lets them remove one.
  - Doors and every other shape stay with the DM.
- **Parsers in `core::actions`:**
  - `parse_note` finds a DC with an ability and any dice.
  - `parse_monster_actions` reads named action paragraphs in the 2024 SRD wording ("Melee Attack Roll: +4 … Hit: 5 (1d6 + 2) Slashing") and the 2014 wording ("Melee Weapon Attack: +4 to hit"), including packed dice (`3d6+5`) and flat damage.
  - On the bundled sources, the parser finds the to-hit of 440/440 SRD attacks, 655/658 in the Black Flag reference and 1041/1043 in the a5e monster manual. It finds damage dice on 439, 657 and 1026 of those attacks; the rest only apply conditions.
- **Acting panel:**
  - Monsters use the parsed actions, with doubled dice offered after a natural 20, and the stat block below.
  - Characters reuse the sheet's play view. A container query stacks its layout in the narrow drawer.
  - The DM opens a combatant from its name in the tracker, or follows the turn. Clicking a linked token switches the open panel.
- **Player preview:** a DM-only `GET /api/campaigns/{campaign}/encounters/{encounter}/as/{player}` runs `permissions::encounter_for` for that player and returns the result. `permissions::can_preview_as` holds the rule.
  - The tracker and the map now read their encounter from a signal. The preview gives them the server's answer, with interactions off.
  - The preview refetches when the DM's copy of the encounters or the campaign settings change.

## Alternatives

- **Filtering on the client for the preview:** instant and offline, but it would copy a permission rule out of the server's permissions module and could drift from what players get.
- **Templates as client-only overlays:** nothing to sync, but other players wouldn't see the area of a spell being aimed.
- **Separate shape types per feature:** clearer per type, but more events and messages. One shape list keeps hiding, moving and filtering in one place.
- **A full sheet page for characters:** the DM would leave the map. The drawer keeps both in view.

## Consequences

- The preview needs a connection.
- Area coverage uses cell centres, a common grid convention. It's a guide: the DM still decides.
- Monster actions that only impose conditions show no roll buttons. The stat block below still has the full text.
