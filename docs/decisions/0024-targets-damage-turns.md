# 0024: Targets, applied damage and turn economy

## Context

The Cast button only spent a slot. The user asked for casting to let the caster pick a target and roll what the caster rolls, never anything for the receiver. They also asked to apply rolled damage to targets, to track actions and movement per turn, and for tracker damage to reach the character sheet. They chose these options:

- only the DM applies damage;
- movement comes from token drags;
- the owner and the DM mark a character's action, bonus action and reaction.

## Decision

- **Roll targets:** `DiceOp::Roll`, `DiceOp::Physical` and `RollRecord` carry `targets: Vec<CombatantId>` (at most 20). The roll label lists the target names.
  - An attack rolls once per target, and the result card offers **Roll damage**, or **Roll critical damage** with doubled dice after a natural 20.
  - A save spell rolls its damage once, with the DC and "half on a success" in the label. Automatic effects roll their damage or healing.
  - Targets never roll.
- **Applying damage:** for the DM, every roll without a d20 gets **Apply…**, with the roll's targets preselected and full, half (rounded down) or double per combatant, as damage or healing.
  - Damage to a character becomes `CharacterChange::Hp` with `HitPoints::apply` (temporary HP first). The sheet is the single source of a character's HP, and the tracker reads it from the sheet or the party summary.
  - Damage to any other combatant stays `EncounterOp::Damage`, which uses the same `HitPoints::apply`.
- **Turn economy:** `Combatant` gains `speed: Option<u16>` and `used: TurnUse { action, bonus, reaction, moved }`.
  - `EncounterOp::UseTurn` (appended) sets it. `permissions::acts_for` lets the DM, or the owner of a character's combatant, use it, the same rule as `SetInitiative`.
  - `next_turn` resets the new current combatant's `used`, so a reaction lasts until that creature's next turn.
  - `Encounter::move_shape` applies every `ShapeMoved` on the server and on each client. It adds 5 ft per square of Chebyshev distance (the 2024 grid rule) when the moved token belongs to the combatant whose turn it is, after round 1 has started. Because each copy computes it from the same event, no extra event is needed.
  - Speed comes from the sheet for characters and from "Speed N ft." in a monster's stat block.
- **Sheet automation:** casting marks the action, bonus action or reaction by the spell's casting time, and a weapon or spell to-hit roll marks the action, when the character has a combatant in the running encounter. The spell list's **Cast** opens the side panel, with its target picker, when the spell has something to roll.
- **Versions:** the combatant and roll layouts changed, so `PROTOCOL_VERSION` is 10 and the client cache schema is 9.
  - The server decodes encounters stored by protocol 8 and 9.
  - Stored rolls that no longer decode are skipped. The log keeps 50 rolls, and they would be replaced within a session anyway.

## Alternatives

- **Players apply their own damage:** faster, but players would need permission to damage combatants. The user kept that with the DM.
- **A combatant HP copy synced to the sheet by the server:** two sources to keep in step. Reading the sheet avoids drift.
- **A separate movement event:** more wire traffic and another projection. Deriving the distance from `ShapeMoved` keeps one event.
- **Rolling saves for targets:** explicitly not wanted. Targets roll their own.

## Consequences

- With several targets, each attack roll gets its own card in the log, and the result card shows the last one.
- Dragging a token back and forth adds every leg. The −5/+5 buttons correct it.
- Players under the hidden combat rule see no combatants, so they can't pick targets.
