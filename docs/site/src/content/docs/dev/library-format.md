---
title: Library file format
description: The beholder-library JSON/YAML format used to import and export the server library.
sidebar:
  order: 6
---

This page is the standard for **Beholder library files, version 1**: the only format the server library imports and exports. A file in another layout (for example the `dnd-data` JSON files) is refused with "This file isn't a Beholder library file" until it's converted; see [Converting dnd-data files](#converting-dnd-data-files).

- **Encoding:** UTF-8, JSON or YAML. The app picks the parser from the file extension: `.yaml` or `.yml` is YAML, anything else JSON.
- **Machine-readable schema:** [`beholder-library-1.schema.json`](/schemas/beholder-library-1.schema.json) (JSON Schema 2020-12). Editors such as VS Code can validate a file against it.
- **Reference implementation:** `crates/client-core/src/library_file.rs`.

## Versioning

- `version` changes only for incompatible changes: a field renamed, removed or given a new meaning.
- New optional fields can be added within version 1. Readers ignore fields they don't know, so older apps keep reading newer version-1 files.
- An app refuses a file whose `version` is newer than it supports, rather than guessing.

```yaml
format: beholder-library
version: 1
entries:
  - kind: spell
    name: Ember Lance
    book: Homebrew
    rules: "2024"
    level: 2
    school: Evocation
    classes: [Sorcerer, Wizard]
    concentration: true
    text: |
      A lance of fire streaks toward a creature you can see within range...
  - kind: monster
    name: Ember Imp
    rules: "2014"
    cr: 1/4
    creature_type: Fiend
    size: Tiny
    text: |
      **AC** 13 · **HP** 10 (3d4 + 3)
```

## Top level

| Field | Value |
|---|---|
| `format` | Always `beholder-library`. Any other value is refused. |
| `version` | `1`. A higher version is refused with "newer than this app supports". |
| `entries` | The list of entries. |

## Every entry

| Field | Required | Value |
|---|---|---|
| `kind` | yes | `spell`, `monster`, `magic-item`, `equipment`, `class`, `subclass`, `species`, `background`, `feat`, `feature`, `trait`, `condition` |
| `name` | yes | 1 to 120 characters |
| `book` | no | 1 to 120 characters |
| `rules` | no | `"2024"` (default), `"2014"` or `level-up` |
| `text` | no | Markdown, up to 64 KB |

## Fields per kind

| Kind | Fields |
|---|---|
| `spell` | `level` (0–9, default 0), `school` (required), `classes` (list), `ritual`, `concentration` (booleans) |
| `monster` | `cr` (required: `0`, `1/8`, `1/4`, `1/2`, `1`–`30`, or a decimal), `creature_type`, `size` (both required) |
| `magic-item` | `rarity`, `category` (both required), `attunement` (boolean) |
| `equipment`, `feat` | `category` (required) |
| `subclass` | `class` (required) |
| `feature` | `class` and `level` (1–20), both required |
| others | none |

Facet strings are limited to 80 characters and a spell to 32 classes. Unknown fields are ignored.

## Import rules

- Each entry becomes a new library entry with a new id, sent as a normal command through the outbox, so an interrupted import resumes after reconnecting.
- An entry with the same kind, name (case-insensitive), book and rules version as one already in the library is skipped.
- An entry that fails validation is reported with its position, name and reason; the others are imported.
- Entries are sent in batches of up to 500 (and about 1 MB), so files with tens of thousands of entries import in a few dozen commands.

## Converting dnd-data files

The JSON files from the `dnd-data` repository (`spells.json`, `monsters.json`, `items.json`, `classes.json`, `species.json`, `backgrounds.json`, each an array of `{ name, description, properties, publisher, book }`) can be converted with:

```sh
cargo xtask dnd-data
```

It reads those files from `data/` and writes one library file per category to `data/converted/` (git-ignored, never committed), then validates every converted entry and prints how many would be rejected and why.

| Source file | Kind | Fields taken from |
|---|---|---|
| `spells.json` | `spell` | `Level` (or "cantrip"), `School`, `Classes` or the "(Ranger, Wizard)" at the start of the description, ritual and concentration from the casting time and duration |
| `monsters.json` | `monster` | `Challenge Rating` (0 to 30, else the description's "Challenge X", else 0), `Size` (the first one), `Type` |
| `items.json` | `magic-item`, or `equipment` without a real rarity | `Item Type`, `Item Rarity`, `Requires Attunement` |
| `classes.json` | `class`, and `subclass` (see below) | none for classes; `class` for subclasses |
| `species.json`, `backgrounds.json` | `species`, `background` | none |

Every entry keeps its `book`. The rules version is `2024` when the book name contains 2024, `level-up` for Level Up books, `2014` otherwise.

### Classes and subclasses

`classes.json` has no separate subclass entries, and the upstream repository has no subclass file. What the file contains:

| Classes from | Subclass content |
|---|---|
| Player's Handbook (2024), Free Basic Rules (2024) | None: the text ends at level 20. |
| Player's Handbook (2014) | Only the names, listed at the end of each class. |
| Free Basic Rules (2014) | One subclass per class with its full rules, appended to the class. |
| Fateforge | Empty descriptions. |
| Other books | Their own classes; any subclass text stays inside the class. |

So the converter:

- turns every `classes.json` entry into a `class`;
- splits the 12 subclasses of the Free Basic Rules (2014) out into `subclass` entries with their parent `class` and book (Path of the Berserker, College of Lore, Life Domain, Circle of the Land, Champion, Way of the Open Hand, Oath of Devotion, Hunter, Thief, Draconic Bloodline, The Fiend, School of Evocation). A subclass starts at its name, found after the first 40% of the class text, not preceded by "the", "a" or "of", and followed by a capitalised word. The class keeps its group heading ("Primal Paths") and a pointer to the subclass;
- turns the 2014 Player's Handbook name lists into a "Subclasses" section, grouped by book, that says only the names are included.

Subclass rules that aren't in the files can be added in the app's editor.

### Text formatting

The descriptions in these files have lost their line breaks and carry scraping leftovers. The converter:

- removes the Roll20 "Free Basic Rules only contain a fraction…" paragraph, invisible word-joiner characters and spaces before commas;
- turns labels written "Name . text" or "Name : text" into bold paragraphs, but keeps attack-line labels (Hit, Miss, Melee Weapon Attack…) inline in italics;
- adds monster section headings (Traits, Actions, Bonus Actions, Reactions, Legendary Actions…);
- turns class features into headings, for both layouts: "Level 3: Barbarian Subclass" (2024) and "Danger Sense At 2nd level…" (2014);
- splits spell header lines (Casting Time, Range, Components, Duration, Classes) and higher-level paragraphs;
- drops the name and type line items repeat at the start of their text.

Text where no structure can be detected stays as one paragraph.
