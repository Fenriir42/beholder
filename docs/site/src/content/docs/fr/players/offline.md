---
title: Mauvaise connexion et jeu hors ligne
description: Ce que signifient l'indicateur et le bandeau de connexion, et ce que deviennent vos modifications hors ligne.
sidebar:
  order: 3
---

Beholder est conçu pour les connexions qui vont et viennent. Tout ce que vous faites est d'abord enregistré sur votre ordinateur, puis envoyé au serveur dès que possible.

## L'indicateur de connexion

L'indicateur de la barre du haut (et en bas de la barre latérale d'une campagne) affiche toujours un point de couleur **et** un mot :

| Indicateur | Signification |
|---|---|
| **En ligne** | Connecté et à jour. |
| **Connexion** | Première connexion après le lancement de l'application. |
| **Reconnexion (n en attente)** | La connexion a été perdue ; Beholder réessaie. `n` modifications attendent d'être envoyées. |
| **Hors ligne (cache)** | Plusieurs tentatives ont échoué. Vous voyez les données enregistrées sur votre ordinateur ; Beholder continue de réessayer en arrière-plan. |

Quand vous n'êtes pas en ligne, un bandeau sous la barre du haut indique que vos modifications sont en attente. Il ne bloque rien. Cliquez sur **Réessayer** pour tenter de vous reconnecter immédiatement (par exemple juste après le retour de votre VPN).

## Ce que deviennent vos modifications

- Quand vous renommez une campagne, changez un réglage ou créez une campagne, vous voyez le résultat **immédiatement**, même hors ligne.
- La modification est enregistrée sur votre ordinateur et envoyée au retour de la connexion. Fermer l'application ne la perd pas.
- Quand le serveur l'accepte, rien ne change à l'écran : elle reste simplement en place.
- Si le serveur la **refuse** (par exemple un joueur qui tente une action réservée au MJ, ou une campagne qui n'existe plus), Beholder l'annule à l'écran et affiche un court message expliquant pourquoi.
- Une modification restée **plus de 7 jours** hors ligne est abandonnée, car le serveur ne peut plus garantir qu'elle ne serait appliquée qu'une seule fois. Un message vous prévient si cela arrive.

## Démarrer sans réseau

Beholder s'ouvre directement sur votre bibliothèque, à partir de ce qu'il a enregistré la dernière fois, sans attendre le serveur. Les couvertures déjà vues sont elles aussi gardées sur votre ordinateur.

## Session expirée

Si vous n'avez pas ouvert Beholder depuis 30 jours, ou si vous vous êtes déconnecté volontairement, l'application vous demande de vous reconnecter. Les données enregistrées pour ce compte restent sur votre ordinateur.
