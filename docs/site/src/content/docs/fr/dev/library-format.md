---
title: Format des fichiers de bibliothèque
description: Le format JSON/YAML beholder-library utilisé pour importer et exporter la bibliothèque du serveur.
sidebar:
  order: 6
---

La bibliothèque du serveur s'importe et s'exporte sous forme de fichier `beholder-library`, en JSON ou en YAML (selon l'extension : `.yaml`/`.yml` pour YAML, tout le reste en JSON). Le code est dans `crates/client-core/src/library_file.rs`.

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

## Niveau supérieur

| Champ | Valeur |
|---|---|
| `format` | Toujours `beholder-library`. Toute autre valeur est refusée. |
| `version` | `1`. Une version supérieure est refusée (« plus récente que ce que l'application gère »). |
| `entries` | La liste des entrées. |

## Toutes les entrées

| Champ | Obligatoire | Valeur |
|---|---|---|
| `kind` | oui | `spell`, `monster`, `magic-item`, `equipment`, `class`, `subclass`, `species`, `background`, `feat`, `feature`, `trait`, `condition` |
| `name` | oui | 1 à 120 caractères |
| `book` | non | 1 à 120 caractères |
| `rules` | non | `"2024"` (par défaut), `"2014"` ou `level-up` |
| `text` | non | Markdown, jusqu'à 64 Ko |

## Champs par type

| Type | Champs |
|---|---|
| `spell` | `level` (0–9, 0 par défaut), `school` (obligatoire), `classes` (liste), `ritual`, `concentration` (booléens) |
| `monster` | `cr` (obligatoire : `0`, `1/8`, `1/4`, `1/2`, `1`–`30`, ou un décimal), `creature_type`, `size` (tous deux obligatoires) |
| `magic-item` | `rarity`, `category` (tous deux obligatoires), `attunement` (booléen) |
| `equipment`, `feat` | `category` (obligatoire) |
| `subclass` | `class` (obligatoire) |
| `feature` | `class` et `level` (1–20), tous deux obligatoires |
| autres | aucun |

Les chaînes des champs sont limitées à 80 caractères et un sort à 32 classes. Les champs inconnus sont ignorés.

## Règles d'import

- Chaque entrée devient une nouvelle entrée de bibliothèque avec un nouvel identifiant, envoyée comme une commande normale via la file d'envoi : un import interrompu reprend après la reconnexion.
- Une entrée de même type, nom (sans tenir compte de la casse), livre et version des règles qu'une entrée existante est ignorée.
- Une entrée invalide est signalée avec sa position, son nom et la raison ; les autres sont importées.
