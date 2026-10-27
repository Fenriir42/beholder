---
title: Format des fichiers de bibliothèque
description: Le format JSON/YAML beholder-library utilisé pour importer et exporter la bibliothèque du serveur.
sidebar:
  order: 6
---

Cette page est la norme des **fichiers de bibliothèque Beholder, version 1** : le seul format que la bibliothèque du serveur importe et exporte. Un fichier d'une autre structure (par exemple les fichiers JSON de `dnd-data`) est refusé avec « Ce fichier n'est pas un fichier de bibliothèque Beholder » tant qu'il n'est pas converti ; voir [Convertir les fichiers dnd-data](#convertir-les-fichiers-dnd-data).

- **Encodage :** UTF-8, JSON ou YAML. L'application choisit selon l'extension : `.yaml` ou `.yml` pour YAML, tout le reste en JSON.
- **Schéma lisible par machine :** [`beholder-library-1.schema.json`](/schemas/beholder-library-1.schema.json) (JSON Schema 2020-12). Des éditeurs comme VS Code peuvent valider un fichier avec.
- **Implémentation de référence :** `crates/client-core/src/library_file.rs`.

## Versions

- `version` ne change que pour une modification incompatible : un champ renommé, supprimé ou dont le sens change.
- De nouveaux champs facultatifs peuvent s'ajouter dans la version 1. Les lecteurs ignorent les champs inconnus : une ancienne application lit toujours un fichier version 1 plus récent.
- Une application refuse un fichier dont la `version` est plus récente que ce qu'elle gère, plutôt que de deviner.

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
- Les entrées sont envoyées par lots de 500 au plus (et environ 1 Mo) : un fichier de plusieurs dizaines de milliers d'entrées s'importe en quelques dizaines de commandes.

## Convertir les fichiers dnd-data

Les fichiers JSON du dépôt `dnd-data` (`spells.json`, `monsters.json`, `items.json`, `classes.json`, `species.json`, `backgrounds.json`, chacun un tableau de `{ name, description, properties, publisher, book }`) se convertissent avec :

```sh
cargo xtask dnd-data
```

La commande lit ces fichiers dans `data/` et écrit un fichier de bibliothèque par catégorie dans `data/converted/` (ignoré par git, jamais publié), puis valide chaque entrée convertie et indique combien seraient refusées et pourquoi.

| Fichier source | Type | Champs issus de |
|---|---|---|
| `spells.json` | `spell` | `Level` (ou « cantrip »), `School`, `Classes` ou le « (Ranger, Wizard) » en début de description, rituel et concentration d'après le temps d'incantation et la durée |
| `monsters.json` | `monster` | `Challenge Rating` (de 0 à 30, sinon le « Challenge X » de la description, sinon 0), `Size` (la première), `Type` |
| `items.json` | `magic-item`, ou `equipment` sans vraie rareté | `Item Type`, `Item Rarity`, `Requires Attunement` |
| `classes.json`, `species.json`, `backgrounds.json` | `class`, `species`, `background` | aucun |

Chaque entrée garde son `book`. La version des règles est `2024` quand le nom du livre contient 2024, `level-up` pour les livres Level Up, `2014` sinon.

Les descriptions de ces fichiers ont perdu leurs retours à la ligne. Le convertisseur rétablit ce qu'il détecte de façon fiable : titres de sections des monstres (Traits, Actions, Legendary Actions…), étiquettes d'actions et de traits en gras écrites « Nom . texte », lignes d'en-tête des sorts (Casting Time, Range, Components, Duration, Classes) et paragraphes de niveau supérieur. Le reste reste en un seul paragraphe.
