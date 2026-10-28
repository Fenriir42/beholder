# 0020: Computed spell and weapon effects on the sheet

## Context

Spell-heavy characters are hard to play from a sheet that only lists names. Players want to see, for each spell and weapon, what the target rolls or what they roll to hit, and how much damage or healing it does at their level and slot, as D&D Beyond shows. The Phase 5 dice system will need the same numbers as roll expressions. Compendium entries hold this only as text, in two styles: the SRD 5.2 tables and sentences, and the scraped 2014 texts in users' libraries.

## Decision

- **`core::actions`** is a small, pure model:
  - `Roll { count, sides, bonus }` displays as dice notation (`8d6`, `1d4+1`). It's the hand-off point for the dice system, which will roll these values.
  - `parse_spell` reads casting time, range, duration and components; an attack ("spell attack") or a saving throw and its ability, whichever the text mentions first; the first damage dice with their type and flat bonus; half damage on a successful save; healing dice and "plus your spellcasting ability modifier"; cantrip scaling at levels 5, 11 and 17; extra dice per slot level and the base level they start above; and darts or beams, with extra ones per slot level or by cantrip tier.
  - `parse_weapon` reads damage, versatile damage, the damage type, finesse and ranged.
- **`spell_effect` and `weapon_effect`** combine that with a character. Spells use the spell save DC or attack of the class that lists the spell, otherwise the first spellcasting class. Damage is scaled for cantrips and upcasting. Weapons use STR, DEX for ranged weapons, or the better of the two for finesse, and assume proficiency. Each returns an `Effect` with a resolution (attack bonus, save ability and DC, or automatic), damage, healing and a repeat count.
- **Parsing runs in the native shell.** The `sheet_details` command reads compendium entries and returns the parsed info, so the WASM interface only carries the small effect calculations. The sheet requests details for every spell and inventory item, linked or matched by name, once each.
- **Display:** spell rows show the effect at the spell's own level. The Actions tab lists equipped weapons, cantrips and prepared spells with their effects. The side panel shows the casting facts and the effect at each slot level the character can cast.
- **Tests** cover the SRD wording, the 2014 wording (saves, upcasting above "3rd", "when you reach 5th level"), Magic Missile's darts, Eldritch Blast's beams, healing with the modifier, and weapon tables in both formats. On the bundled and converted data, every SRD spell gets its casting facts, and 154 of 339 have a check detected.

## Alternatives

- **A full rules engine with structured spell data:** exact, but SRD and library entries are text, and a rules engine is out of scope (SPEC §5.1).
- **Parsing in the interface:** simpler plumbing, but it would put text parsing in the WASM for no benefit.
- **Precomputing details into the compendium bundles:** faster at runtime, but it would change the bundle format and couldn't cover library entries edited later.

## Consequences

- Spells whose damage isn't written as dice ("takes damage equal to…"), and weapons without a damage line, show no computed effect, only their text.
- Weapon proficiency isn't tracked yet: weapon attacks assume the character is proficient.
