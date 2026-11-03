---
title: Combat
description: Prepare encounters, run initiative, track HP and conditions, and move tokens on the battle map.
sidebar:
  order: 6
---

Open **Combat** in the campaign sidebar. The turn order is on the left and the battle map is on the right (on a narrow window, the map goes below the turn order).

## For the DM

### Encounters

The DM can prepare as many encounters as they like: type a name and click **New encounter**, then switch between them with the **Encounter** list. Players see nothing of an encounter until the DM clicks **Run for the players**. Only one encounter runs at a time, so running one stops the others. **Stop running** takes it away from the players again, and **Delete encounter** asks for confirmation before deleting it.

### Combatants

Under **Add combatants**:

- **Add the party** adds every player's active character, with the HP and AC from their sheet.
- **Search monsters** looks through the compendium sources enabled for the campaign. **Add** fills in HP and AC from the stat block, and numbers copies ("Goblin 2").
- **Quick add** takes a name, HP and AC for anything else.

On each row, type a damage or healing amount and click **Damage** or **Heal**. Temporary HP is used up first. **Edit** opens conditions (pick from the list or type your own, with an optional number of rounds), concentration, max HP, AC, **Hidden from players**, and removal. Click a condition to remove it.

### Initiative and turns

Type an initiative value or click the die next to it. The roll goes through the dice log with the creature's bonus: Dexterity and initiative bonuses from the sheet for characters, the stat block's initiative for monsters. **Roll monster initiative** rolls every combatant that isn't a character and has no initiative yet, as a roll to the DM. The list sorts itself by initiative.

**Start combat** begins round 1 with the first combatant. **Next turn** and **Previous turn** move through the order, and **Reset rounds** goes back to before the fight. A condition with a number of rounds counts down at the end of its creature's turn and disappears at zero. The current turn is marked with ▶ and "their turn", not just a colour.

### What players see

The campaign rule chosen in **Settings** decides how much of the turn order players get:

- **Exact:** everything except hidden combatants, including monster HP and AC.
- **Vague:** monsters show Healthy, Bloodied, Near death or Down instead of HP and AC. Characters still show their HP.
- **Hidden:** players get the map but no turn order.

The server applies this rule before sending anything, so the hidden values never reach the players' apps.

### The map

- **Wall** and **Zone**: pick the tool, then click a cell to place one. The tool stays on until you pick **Select**.
- **Place a token for…** lists the combatants without a token. Pick one and click a cell. A character's token gets its player's colour and belongs to that player.
- **Select** a shape to change its label, width, height or colour, hide it from the players, or remove it. Drag any shape to move it.
- **Width** and **Height** set the grid size, up to 200 by 200 cells.
- **Lock the map for players** stops players from moving their tokens. **Show moves while dragging** lets everyone see a token move as it's dragged, not only when it's let go.

Hidden shapes look faded with a dashed outline for the DM. Players never receive them.

## For players

Once the DM runs an encounter, you see its turn order, if the campaign shows it, and its map. Your own token has a thick outline. Drag it to move it: by default the others see it when you let go, or as you drag if the DM turned on live moves. You can type or roll the initiative of your own character. Everything else is the DM's.

Your token shows your character's initials, so you can find it without relying on colour. Use **+** and **−** to zoom the map.
