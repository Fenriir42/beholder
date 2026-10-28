---
title: Bibliothèque du serveur
description: Ajouter les entrées propres à votre groupe (création maison et contenu des livres que vous possédez), les modifier, les importer et les exporter.
sidebar:
  order: 5
---

La bibliothèque du serveur contient les entrées de compendium propres à votre groupe : création maison et contenu des livres que vous possédez. Elle reste sur votre serveur et n'est jamais livrée avec l'application. Tous les utilisateurs du serveur la partagent.

## Ajouter et modifier

Dans le **Compendium**, cliquez sur **Nouvelle entrée**. Choisissez le **type** (sort, monstre, objet…), puis remplissez :

- le **nom**, et le **livre** d'où vient l'entrée (par exemple « Player's Handbook (2024) » ou « Création maison ») ;
- la **version des règles** : 2024, 2014 ou Level Up ;
- les champs utilisés par la recherche et les filtres : niveau, école et classes d'un sort, facteur de puissance, type et taille d'un monstre, rareté et catégorie d'un objet, etc. ;
- le **texte**, en Markdown. Un nouveau monstre ou sort part d'un modèle à compléter.

Tout utilisateur du serveur peut ajouter des entrées. Une entrée peut être modifiée ou supprimée par son auteur et par toute personne MJ d'une campagne. Si quelqu'un d'autre a enregistré l'entrée pendant que vous la modifiiez, votre enregistrement est refusé et Beholder vous demande de la recharger d'abord : personne n'écrase les modifications d'un autre sans le savoir.

## Coller depuis un livre ou une page

Pour ajouter des entrées d'un livre ou d'une page que vous possédez, cliquez sur **Coller** dans le **Compendium**. Indiquez une seule fois le **livre** et la **version des règles** pour tout le collage, puis collez une entrée ou une section entière, par exemple tous les sorts d'un chapitre, et cliquez sur **Lire les entrées**.

Beholder découpe le texte en entrées. Chaque entrée commence par son nom sur une ligne, suivi d'une ligne qu'il reconnaît :

| Type | Ligne d'en-tête après le nom |
|---|---|
| Sort | « Level 3 Evocation (Sorcerer, Wizard) », « 3rd-level evocation » ou « Evocation Cantrip » |
| Monstre | « Small Fey (Goblinoid), Chaotic Neutral » ; le facteur de puissance vient de la ligne « CR » ou « Challenge » |
| Objet magique | « Wondrous Item, Uncommon (Requires Attunement) », ou une ligne « Rarity » juste en dessous |
| Don | « Origin Feat », « General Feat (Prerequisite: …) » ou « Prerequisite: … » |
| Sous-classe | un nom écrit « Sorcerer: Storm Sorcery », ou une ligne comme « Fighter subclass: … » |

Le reste de chaque entrée devient du texte mis en forme : les lignes courtes suivies d'un paragraphe deviennent des titres, les paragraphes « Nom. Texte » ont leur nom en gras, les statistiques, caractéristiques et sections des monstres (Actions, Reactions…) sont présentées comme dans le SRD, les puces deviennent des listes et les lignes séparées par des tabulations deviennent des tables. Le texte qui ne correspond à aucune entrée est gardé comme une seule entrée du **type si non détecté** choisi.

Vous relisez ensuite la liste. Chaque entrée affiche ses champs, avec la mention **Vérifier les champs** quand quelque chose a été deviné (un monstre sans facteur de puissance, une entrée du type par défaut) ou **Déjà dans la bibliothèque** quand une entrée de même type, nom, livre et version des règles existe. Les doublons sont décochés au départ. **Aperçu** affiche le texte mis en forme, et **Modifier** ouvre l'entrée dans l'éditeur ; l'enregistrer l'ajoute tout de suite. Cliquez ensuite sur **Accepter la sélection** ou **Tout accepter**. Cochez **Remplacer les entrées déjà dans la bibliothèque** pour écraser les doublons que vous avez le droit de modifier.

## Priorité sur les autres sources

En cas de nom en double, les entrées de la bibliothèque passent en premier : si votre bibliothèque contient « Fireball », le compendium affiche la vôtre, la version du SRD étant listée sous **Aussi dans**.

## Dans une campagne

La bibliothèque est une source de règles comme un livre : elle est activée par défaut dans les nouvelles campagnes, et le MJ peut la désactiver dans **Paramètres → Sources de règles**.

Le MJ peut aussi masquer une entrée aux joueurs d'une campagne : ouvrez l'entrée et cliquez sur **Masquer dans cette campagne**. Les joueurs de cette campagne ne la voient plus ; le MJ la voit toujours, marquée **Masquée**, et peut la réafficher.

## Import et export

- **Exporter en JSON** ou **Exporter en YAML** enregistre toute la bibliothèque dans votre dossier Téléchargements et affiche le nom du fichier.
- **Importer** lit un fichier au même format. Les entrées déjà présentes (même type, nom, livre et version des règles) sont ignorées, sauf si vous cochez **Remplacer les entrées déjà dans la bibliothèque** : elles sont alors mises à jour avec la version du fichier, là où vous avez le droit de les modifier (vos propres entrées, ou toutes si vous êtes MJ d'une campagne). Les entrées qui posent problème sont listées avec la raison, et les autres sont importées.

Le format est décrit dans [Format des fichiers de bibliothèque](/fr/dev/library-format/). Les imports de plusieurs dizaines de milliers d'entrées ne posent pas de problème : elles partent par lots, et un bandeau indique combien sont encore en route vers le serveur.

### Importer les fichiers dnd-data

Les fichiers d'une autre structure sont refusés tant qu'ils ne sont pas convertis. Pour les fichiers JSON de `dnd-data` rangés dans `data/`, lancez `cargo xtask dnd-data` dans le dossier du projet : la commande écrit des fichiers prêts à importer dans `data/converted/` (un par catégorie). Importez-les un par un depuis le Compendium. Quand le convertisseur s'améliore, relancez-le et réimportez en cochant **Remplacer les entrées déjà dans la bibliothèque** pour mettre à jour ce que vous aviez importé. Voir [Convertir les fichiers dnd-data](/fr/dev/library-format/#convertir-les-fichiers-dnd-data) pour le détail.
