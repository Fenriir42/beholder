---
title: Mises à jour, sauvegardes et journaux
description: L'exploitation au quotidien d'un serveur Beholder.
sidebar:
  order: 2
---

## Mettre à jour

```sh
cd ~/beholder
docker compose pull
docker compose up -d
```

Les joueurs connectés se reconnectent automatiquement au bout de quelques secondes. Les modifications faites pendant le redémarrage sont envoyées dès leur retour en ligne.

Si le nouveau serveur utilise un protocole plus récent, les anciennes applications affichent *Mettez Beholder à jour* et continuent de fonctionner hors ligne jusqu'à leur mise à jour.

Pour rester sur une version donnée, mettez `BEHOLDER_TAG=0.1.0` dans `.env`.

## Où sont les données

Tout se trouve dans le volume Docker `beholder-data` :

| Chemin dans le volume | Contenu |
|---|---|
| `beholder.db` (+ `-wal`, `-shm`) | Base SQLite : comptes, sessions, campagnes, journal d'événements |
| `images/ab/<hash>-thumb.webp` | Miniatures des couvertures (environ 480 px de large) |
| `images/ab/<hash>-display.webp` | Couvertures en taille d'affichage (1920 px max) |

## Sauvegarder

La sauvegarde la plus sûre arrête le serveur quelques secondes :

```sh
docker compose stop server
docker run --rm -v beholder_beholder-data:/data -v "$PWD":/backup alpine \
  tar czf /backup/beholder-$(date +%F).tar.gz -C /data .
docker compose start server
```

Le volume s'appelle `<dossier>_beholder-data` ; vérifiez avec `docker volume ls`. Pour restaurer, arrêtez le serveur, extrayez l'archive dans le volume de la même manière, puis redémarrez-le.

## Journaux

```sh
docker compose logs -f server
```

Mettez `BEHOLDER_LOG=debug` dans `.env` (puis `docker compose up -d`) pour plus de détails ; la valeur par défaut est `info`.

## Maintenance automatique

Une fois par jour, le serveur :

- garde les 1 000 derniers événements de chaque campagne (l'historique plus ancien est résumé ; une application restée très longtemps hors ligne reçoit simplement l'état actuel),
- oublie les accusés de commandes de plus de 8 jours,
- supprime les sessions inutilisées depuis 30 jours.
