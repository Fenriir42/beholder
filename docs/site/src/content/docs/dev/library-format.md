---
title: Library file format
description: The beholder-library JSON/YAML format used to import and export the server library.
sidebar:
  order: 6
---

The server library is imported and exported as a `beholder-library` file, in JSON or YAML (chosen from the file extension: `.yaml`/`.yml` is YAML, anything else JSON). The code is `crates/client-core/src/library_file.rs`.

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
