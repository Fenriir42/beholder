---
title: Character sheets
description: Create characters, fill in an assisted sheet, and see what the rest of the party sees.
sidebar:
  order: 4
---

Each player can have several characters in a campaign, with one **active** at a time: the active character is the one shown to the party (and, later, used by the combat tracker and the dice).

## Creating a character

On the campaign home, open the **Characters** tab. Type a name, pick the rules version (2024 or 2014), and click **New character**: the sheet opens. Your first character is active; use **Set active** to switch to another one. **Character sheet** in the campaign sidebar always opens your active character.

## Playing with the sheet

The sheet opens in a play view, laid out like D&D Beyond:

- **Top:** AC, initiative, speed, proficiency, and hit points. Type an amount and click **Damage** (temporary HP go first), **Heal**, or **Set temp**. The six ability modifiers sit underneath, with conditions and exhaustion when you have any.
- **Left:** saving throws and skills with their bonuses (○ not proficient, ● proficient, ◆ expertise) and passive scores.
- **Tabs:**
  - **Actions:** your equipped items, limited-use resources with − and + buttons, hit dice and death saves.
  - **Spells:** spell save DC and attack, spell slots as dots you click to spend (●) or restore (○), level filters and a search box. Each spell shows its school, concentration and ritual when it's in the compendium, and the dot before it marks it prepared. **Add spells** lists your classes' spells up to the highest level you can cast. Untick the filter to see every spell, or add one that isn't in the compendium as typed.
  - **Inventory:** money, and items with quantity − and + and an "equipped" box. **Add items** searches magic items and equipment.
  - **Features & traits:** species, background, classes and subclasses, feats and features. **Add features and feats** searches the compendium.
  - **Notes:** free text.
- **Right:** clicking any spell, item, feature, species or class opens its compendium entry beside the sheet, with **Prepare**/**Unprepare** and **Remove** where they apply. Names typed without a link (for example from an imported file) are matched to the compendium by name. On narrow windows the panel slides over the sheet.

### Computed effects

When a spell or weapon is in the compendium, the sheet works out what it does for your character:

- **Spells:** the attack bonus ("Attack +6"), or the saving throw the target makes and your DC ("DEX save DC 14", with "half on a success" when it applies), and the damage or healing dice at the spell's level, for example "2d10 fire" for Fire Bolt at level 5 or "Heals 2d8+3" for Cure Wounds. Cantrips scale with your level, Magic Missile and Eldritch Blast show how many darts or beams you get, and the side panel lists the damage at every slot level you can cast.
- **Weapons:** the attack bonus and damage with your modifier (STR, DEX for ranged weapons, the better of the two for finesse weapons), plus the two-handed damage of versatile weapons. Weapon proficiency isn't tracked yet, so attacks assume you're proficient.

The **Actions** tab gathers your equipped weapons, cantrips and prepared spells with these numbers, ready for the dice to come. Spells whose damage isn't written as dice show only their text.

### Resting and casting

- **Short rest** and **Long rest** at the top ask for a confirmation, then apply the rest for you. A short rest brings back the resources set to come back on a short rest, and warlock pact slots. Spend hit dice yourself with **Heal**. A long rest restores your HP, every spell slot, the short- and long-rest resources and your hit dice (all of them under the 2024 rules, half your level and at least one under 2014), clears death saves, lowers exhaustion by one and ends concentration. Each resource's "comes back on" setting is in **Edit character**.
- **Cast** next to a spell with something to roll opens the side panel: pick a slot level and, during a fight, the **Targets**, then **Cast** spends the slot and rolls your side (the attack for each target, or the damage or healing). A spell with nothing to roll is cast straight from the list. Either way the slot of the lowest level you have left (or a pact slot) is the default, the button is greyed out when no slot can cast it, cantrips are free, and in a running encounter your action, bonus action or reaction is marked by the casting time. See [Combat](../combat/) for targets and damage.
- **Concentration:** casting a concentration spell shows "Concentrating on…" at the top, with **End concentration**. If you're already concentrating on another spell, **Cast** opens the side panel, where the button says which spell it will end.
- **Weapon proficiency:** in a weapon's side panel, untick **Proficient with this weapon** and its attack bonus no longer includes your proficiency bonus.

## Levelling up

**Level up** at the top of the sheet walks you through a new level:

1. **Class:** the class you have most levels in is picked; choose another, or **New class (multiclass)…** and search the compendium.
2. **Features:** the features the class table (and your subclass) give at the new class level, all ticked. Untick any you don't want added. If the table lists cantrips, prepared or known spells, you see how they change; spell slots update by themselves, and you add the new spells from the Spells tab afterwards.
3. **Subclass:** when the table says this is the subclass level and you have none yet, pick one from the campaign's sources.
4. **Ability Score Improvement:** +2 to one ability, +1 to two, or a feat instead. Scores don't go above 20 this way. At level 19, the Epic Boon asks for a feat.
5. **Hit points:** take the average, **Roll** the hit die (through the dice log), or type a physical roll. The gain adds your Constitution modifier, at least 1. If you raise Constitution, earlier levels get the extra hit point too. Your current HP goes up by the same amount.

**Level up** applies everything at once. You can still change anything afterwards in **Edit character**.

## Editing the character

**Edit character** at the top opens the build view: name and rules version, species and background, classes (level, hit die, kind of spellcaster, spellcasting ability), ability scores and saving throw proficiencies, skill proficiencies, armor and speed, maximum HP, resources, feats and features, plus **Export** and **Delete**. Click **Done** to go back to the play view. Every field saves as soon as you leave it, even offline (changes are sent when the connection comes back).

The app computes:

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
