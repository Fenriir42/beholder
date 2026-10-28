---
title: Character sheets
description: Create characters, fill in an assisted sheet, and see what the rest of the party sees.
sidebar:
  order: 4
---

Each player can have several characters in a campaign, with one **active** at a time: the active character is the one shown to the party (and, later, used by the combat tracker and the dice).

## Creating a character

On the campaign home, open the **Characters** tab. Type a name, pick the rules version (2024 or 2014), and click **New character**: the sheet opens. Your first character is active; use **Set active** to switch to another one. **Character sheet** in the campaign sidebar always opens your active character.

## The sheet

Every field saves as soon as you leave it, even offline (changes are sent when the connection comes back). The app computes:

- ability modifiers, saving throws and skills, with proficiency, **expertise** and **Jack of All Trades**;
- passive Perception, Insight and Investigation, and initiative;
- the proficiency bonus from your total level;
- **AC** from the formula you choose: no armor (10 + DEX), armor (base + DEX up to a cap), Unarmored Defense (10 + DEX + another ability), natural armor, or a fixed value, plus a shield and other bonuses;
- spell save DC and spell attack for each class with a spellcasting ability;
- **spell slots** from your classes, including the multiclass table and warlock pact slots;
- the average maximum HP for your hit dice, which you can apply with one click.

**Classes:** add a class and pick it from the compendium. For the standard classes, the hit die, the kind of spellcaster and the spellcasting ability are filled in for you; for others, set them yourself. The kinds of spellcaster are full, half (paladin, ranger), half rounded up (artificer), third (Eldritch Knight, Arcane Trickster) and pact magic (warlock). 2014 half casters get slots from level 2, 2024 ones from level 1.

**Species, background, feats, features, spells and items** can be picked from the campaign's compendium as you type (marked "from the compendium"), or typed freely for anything that isn't in it.

You also track HP (current, maximum, temporary), hit dice spent, death saves, conditions, exhaustion (in 2024, each level is −2 to every d20 test), spell slots spent, resources such as sorcery points or rages, inventory and money, and free notes.

Leveling up is manual: change a class level and everything above is recomputed. The app doesn't apply class features for you.

## Export and import

**Export** at the bottom of the sheet saves the character to your Downloads folder as a `beholder-character` JSON file (version 1), for a backup or to bring the character to another campaign or server. **Import a character…** in the Characters tab picks such a file and adds it as a new character of yours in this campaign.

## Who sees what

- **You and the DM** see and edit your full sheet. The DM can fix anything, for example to apply damage.
- **The other players** only see a summary in the **Party** tab: the active character's name, classes and levels, AC and HP. The rest of your sheet (notes, inventory, spells…) never reaches their app.
