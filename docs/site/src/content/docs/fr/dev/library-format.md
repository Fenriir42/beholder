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
| `classes.json` | `class`, et `subclass` (voir plus bas) | aucun pour les classes ; `class` pour les sous-classes |
| `species.json`, `backgrounds.json` | `species`, `background` | aucun |

Chaque entrée garde son `book`. La version des règles est `2024` quand le nom du livre contient 2024, `level-up` pour les livres Level Up, `2014` sinon.

### Classes et sous-classes

`classes.json` ne contient pas d'entrées de sous-classes séparées, et le dépôt d'origine n'a pas de fichier de sous-classes. Ce que contient le fichier :

| Classes tirées de | Contenu des sous-classes |
|---|---|
| Player's Handbook (2024), Free Basic Rules (2024) | Aucun : le texte s'arrête au niveau 20. |
| Player's Handbook (2014) | Seulement les noms, listés à la fin de chaque classe. |
| Free Basic Rules (2014) | Une sous-classe par classe avec ses règles complètes, ajoutée à la suite de la classe. |
| Fateforge | Descriptions vides. |
| Autres livres | Leurs propres classes ; un éventuel texte de sous-classe reste dans la classe. |

Le convertisseur, donc :

- fait de chaque entrée de `classes.json` une `class` ;
- sépare les 12 sous-classes des Free Basic Rules (2014) en entrées `subclass` avec leur `class` parente et leur livre (Path of the Berserker, College of Lore, Life Domain, Circle of the Land, Champion, Way of the Open Hand, Oath of Devotion, Hunter, Thief, Draconic Bloodline, The Fiend, School of Evocation). Une sous-classe commence à son nom, trouvé après les premiers 40 % du texte de la classe, non précédé de « the », « a » ou « of », et suivi d'un mot en majuscule. La classe garde son titre de groupe (« Primal Paths ») et un renvoi vers la sous-classe ;
- transforme les listes de noms du Player's Handbook (2014) en section « Subclasses », regroupée par livre, qui précise que seuls les noms sont inclus.

Les règles de sous-classes absentes des fichiers peuvent être ajoutées avec l'éditeur de l'application.

### Mise en forme du texte

Les descriptions de ces fichiers ont perdu leurs retours à la ligne et gardent des restes de l'extraction. Le convertisseur :

- retire le paragraphe Roll20 « Free Basic Rules only contain a fraction… », les caractères invisibles de jonction et les espaces avant les virgules ;
- transforme les étiquettes écrites « Nom . texte » ou « Nom : texte » en paragraphes en gras, mais garde les étiquettes des lignes d'attaque (Hit, Miss, Melee Weapon Attack…) en ligne, en italique ;
- ajoute les titres de sections des monstres (Traits, Actions, Bonus Actions, Reactions, Legendary Actions…) ;
- transforme les aptitudes de classe en titres, pour les deux présentations : « Level 3: Barbarian Subclass » (2024) et « Danger Sense At 2nd level… » (2014) ;
- sépare les lignes d'en-tête des sorts (Casting Time, Range, Components, Duration, Classes) et les paragraphes de niveau supérieur ;
- retire la ligne de nom et de type que les objets répètent au début de leur texte.

Le texte sans structure détectable reste en un seul paragraphe.
