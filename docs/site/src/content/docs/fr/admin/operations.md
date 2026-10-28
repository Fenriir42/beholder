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

## Import depuis un dépôt externe

Pour remplir la bibliothèque du serveur depuis un dépôt de fichiers JSON organisé comme [`nick-aschenbach/dnd-data`](https://github.com/nick-aschenbach/dnd-data) (un fichier par catégorie : `spells.json`, `monsters.json`, `items.json`, `classes.json`, `species.json`, `backgrounds.json`), indiquez ceci dans `.env` et lancez `docker compose up -d` :

```sh
BEHOLDER_IMPORT_REPO=https://github.com/proprietaire/depot
BEHOLDER_IMPORT_REF=HEAD
```

Au démarrage suivant, le serveur télécharge l'archive `.tar.gz` du dépôt en HTTPS (adresses de type GitHub, GitLab, Gitea et Codeberg, ou lien direct vers un `.tar.gz`), lit ses fichiers JSON un enregistrement à la fois, les convertit comme `cargo xtask dnd-data`, et les ajoute à la bibliothèque en un seul import. Les entrées déjà présentes (même type, nom, livre et version des règles) sont ignorées. Cela ne se fait qu'une fois : après un import réussi, il n'est pas refait, même si les variables restent définies. En cas d'échec (pas de réseau, mauvaise adresse), le journal en donne la raison et l'import est retenté au démarrage suivant. `BEHOLDER_IMPORT_REF` est une branche ou une étiquette ; `HEAD` désigne la branche par défaut sur GitHub, et GitLab demande un nom de branche.

Les entrées importées appartiennent au serveur et non à un utilisateur : seul un MJ peut les modifier. Un MJ peut aussi annuler tout l'import depuis **Imports récents** dans le Compendium. Le contenu reste sur votre serveur et n'est jamais livré avec l'application ; n'importez que du contenu que vous avez le droit d'utiliser.

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
