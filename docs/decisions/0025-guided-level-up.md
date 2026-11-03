# 0025: Guided level-up

## Context

Phase 4 ended with an optional guided level-up still to do. Character sheets are assisted, not a rules engine (SPEC §5.1). Every class entry in the bundled sources has its features table: level, features, cantrips, prepared or known spells, slots. Subclass entries have "Level N: Feature" sections. The user chose this as the next step.

## Decision

- **`core::progression`:**
  - `parse_class` reads the hit die and the class table. The header row starts with "Level" and has a "Features" column. Every column that mentions cantrips, prepared or known is kept as a count.
  - `parse_sections` reads a subclass's "Level N: Name" headings.
  - `milestone` recognises the subclass, Ability Score Improvement (or Black Flag's "Improvement") and Epic Boon rows.
  - Every class and subclass in the SRD, Black Flag, Open5e 2024 and the a5e book bundles parses.
  - The shell adds the result to `Details::progression` for class and subclass entries, as it does for spells and monsters.
- **`CharacterChange::LevelUp(Box<LevelUp>)`**, appended. One change carries the class (an existing one, or a new class at level 0), an optional subclass, the hit die result, the features to add, the ability increases and an optional feat. `Character::level_up` applies it all at once:
  - +1 to that class, or adds the new class at level 1;
  - ability increases only when they total at most +2 with at most +2 each, never above 20;
  - hit points: the result clamped to the die, plus the new Constitution modifier, at least 1. A raised modifier also counts for every earlier level, and a first level takes the full die. The gain goes to both max and current HP;
  - features and the feat without duplicates;
  - nothing at level 20.

  The usual owner or DM permission applies, since this is an ordinary character change.
- **Wizard:** the sheet's **Level up** opens it in place of the play view. It is taken from a snapshot of the character, so live updates don't reset it while open.
  - Hit points come from the average, a server roll of the hit die (the result fills in when it arrives), or a typed value.
  - For a new class, the hit die comes from its table, then from `rules::class_defaults`.
  - Spells are only reported (counts before and after). Slots follow from class levels already.

## Alternatives

- **A series of existing changes (Classes, Hp, Features, Abilities):** no new variant, but a dropped connection could apply half a level.
- **A full rules engine** (every class's choices, spell lists and proficiencies): out of scope for an assisted sheet.
- **Rolling HP on the client:** quicker, but not in the shared log like every other roll.

## Consequences

- Features are added by name and resolved against the compendium like any other feature.
- Choices the table doesn't encode (Fighting Style, Expertise skills, Invocations, Metamagic) are features to fill in from **Edit character**.
- Classes without a table get no feature list, but the level and hit points still work.
