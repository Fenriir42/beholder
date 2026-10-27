---
title: Notes et documents
description: Écrire des notes en Markdown, choisir qui les voit, révéler des documents et gérer les copies de conflit.
sidebar:
  order: 3
---

Ouvrez **Notes** dans la barre latérale de la campagne pour voir toutes les notes que vous pouvez lire, et écrire les vôtres. L'onglet **Notes de campagne** de l'accueil de la campagne ne montre que les notes partagées avec tout le groupe.

## Écrire

Cliquez sur **Nouvelle note**. Les notes s'écrivent en Markdown :

| Vous tapez | Vous obtenez |
|---|---|
| `# Titre`, `## Section` | Des titres |
| `**gras**`, `*italique*`, `~~barré~~` | **gras**, *italique*, ~~barré~~ |
| `- élément` ou `1. élément` | Des listes |
| `> texte` | Une citation |
| `| a | b |` avec une ligne `|---|---|` en dessous | Un tableau |

Passez de **Modifier** à **Aperçu** avec les boutons au-dessus du texte. Beholder enregistre tout seul environ une seconde après la fin de la frappe, ainsi que quand vous quittez le champ ou changez de note. L'indicateur à côté de l'auteur affiche **Enregistrement…** ou **Enregistrée**.

**Insérer une image** envoie une image JPEG, PNG ou WebP (jusqu'à 20 Mo) sur votre serveur et l'ajoute à l'endroit du curseur. Seules les images de votre serveur s'affichent, jamais celles d'autres sites. Les liens sont soulignés avec leur adresse, mais ne s'ouvrent pas depuis l'application.

## Qui peut voir une note

Chaque note a un public, affiché par une étiquette sur la note :

| Étiquette | Qui peut la voir | Qui peut la modifier |
|---|---|---|
| **Privée** | Vous seul | Vous seul |
| **MJ uniquement** | Une note privée écrite par le MJ | Le MJ |
| **Groupe** | Tous les membres de la campagne | Tous les membres |
| **Partagée** | Vous et les membres choisis | Vous et ces membres |

Seul l'auteur peut changer le public ou supprimer une note. Cliquez sur **Qui peut la voir** pour le changer.

Si le MJ a activé **Le MJ peut lire les notes privées des joueurs** dans les réglages de la campagne, il peut aussi *lire* (pas modifier) vos notes privées. Chaque joueur peut vérifier ce réglage dans **Règles de la campagne**.

C'est le serveur qui masque : une note que vous ne pouvez pas voir n'est jamais envoyée sur votre ordinateur.

## Révéler un document

L'auteur d'une note voit un bouton **Révéler au groupe**. Un clic la partage avec tout le monde, et elle apparaît aussitôt sur leurs écrans. Pour la masquer à nouveau, remettez le public sur **Moi seul** : elle disparaît chez les autres.

## Copies de conflit

Deux personnes peuvent modifier une note partagée en même temps. Si le serveur reçoit votre modification après celle de quelqu'un d'autre, il ne la jette pas : il la garde dans une **copie de conflit**, privée, et l'originale garde le texte de l'autre personne. Un bandeau sur l'originale mène à votre copie. Reportez ce qu'il faut dans l'originale, puis supprimez la copie.

Si vous tapez pendant que quelqu'un d'autre modifie la note, un avertissement vous le signale avant l'enregistrement.

## Hors ligne

Les notes fonctionnent hors ligne comme le reste : vous pouvez les créer et les modifier, et elles sont envoyées au retour de la connexion. Voir [Mauvaise connexion et jeu hors ligne](/fr/players/offline/).
