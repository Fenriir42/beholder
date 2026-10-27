---
title: Server library
description: Add your group's own entries (homebrew and content from books you own), edit them, import and export them.
sidebar:
  order: 5
---

The server library holds your group's own compendium entries: homebrew, and content from books you own. It lives on your server only and is never bundled with the app. Everyone on the server shares it.

## Adding and editing

In the **Compendium**, click **New entry**. Choose the **type** (spell, monster, item…), then fill in:

- **Name**, and the **book** it comes from (for example "Player's Handbook (2024)" or "Homebrew").
- **Rules version**: 2024, 2014 or Level Up.
- The fields that search and filters use: a spell's level, school and classes, a monster's challenge rating, type and size, an item's rarity and category, and so on.
- The **text**, in Markdown. A new monster or spell starts from a skeleton you can fill in.

Anyone on the server can add entries. An entry can be edited or deleted by its author, and by anyone who is DM of a campaign. If someone else saved the entry while you were editing it, your save is refused and Beholder asks you to reload it first, so nobody's changes are silently overwritten.

## Priority over other sources

Library entries come first when names clash: if your library has a "Fireball", the compendium shows yours, with the SRD version listed under **Also in**.

## In a campaign

The library is a rules source like a book: it's on by default in new campaigns, and the DM can turn it off under **Settings → Rules sources**.

The DM can also hide single entries from the players of one campaign: open the entry and click **Hide in this campaign**. Players of that campaign no longer see it; the DM still does, marked **Hidden**, and can show it again.

## Import and export

- **Export JSON** or **Export YAML** saves the whole library to your Downloads folder and shows the file name.
- **Import** reads a file in the same format. Entries already in the library (same type, name, book and rules version) are skipped. Entries with a problem are listed with the reason, and the rest are imported.

The format is described in [Library file format](/dev/library-format/). Imports of tens of thousands of entries are fine: they're sent in batches, and a banner shows how many are still on their way to the server.

### Importing the dnd-data files

Files in another layout are refused until they're converted. For the `dnd-data` JSON files kept in `data/`, run `cargo xtask dnd-data` in the project folder: it writes ready-to-import files to `data/converted/` (one per category). Import them one at a time from the Compendium. See [Converting dnd-data files](/dev/library-format/#converting-dnd-data-files) for what's converted and how.
