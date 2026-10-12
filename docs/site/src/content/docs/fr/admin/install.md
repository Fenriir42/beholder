---
title: Installer le serveur
description: Faire tourner le serveur Beholder sur un petit VPS avec Docker et Caddy.
sidebar:
  order: 1
---

Un serveur héberge un groupe. Il est conçu pour un VPS Linux avec **1 Go de RAM** et consomme très peu :

| | Mesuré |
|---|---|
| Mémoire du serveur au repos | environ 6 Mo |
| Mémoire du serveur au pic (réencodage d'une photo de 40 mégapixels) | environ 157 Mo |
| Binaire du serveur | environ 6 Mo |

Le VPS ne compile jamais rien. L'image est construite par GitHub Actions et le VPS se contente de la télécharger.

## Prérequis

- Un VPS Linux avec Docker et le plugin Docker Compose.
- Un nom de domaine qui pointe vers le VPS (un enregistrement `A`, par exemple `dnd.exemple.fr`).
- Les ports **80** et **443** ouverts. Caddy utilise le 80 pour obtenir le certificat TLS.

## 1. Ajouter un fichier d'échange (swap)

Un petit fichier d'échange protège le VPS si la mémoire vient à manquer :

```sh
sudo fallocate -l 1G /swapfile && sudo chmod 600 /swapfile
sudo mkswap /swapfile && sudo swapon /swapfile
echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
```

## 2. Copier les fichiers de déploiement

Copiez `deploy/docker-compose.yml` et `deploy/Caddyfile` du dépôt dans un dossier du VPS (par exemple `~/beholder`), puis créez `.env` à côté :

```sh
BEHOLDER_DOMAIN=dnd.exemple.fr
GHCR_OWNER=votre-utilisateur-github-en-minuscules
# BEHOLDER_TAG=0.1.0   # figer une version ; latest par défaut
```

Si l'image est privée sur GitHub, connectez-vous une fois avec un jeton ayant le droit `read:packages` :

```sh
docker login ghcr.io -u votre-utilisateur-github
```

## 3. Démarrer

```sh
docker compose up -d
docker compose logs -f
```

Caddy obtient un certificat automatiquement. Les joueurs saisissent ensuite `https://dnd.exemple.fr` (ou simplement `dnd.exemple.fr`) comme adresse du serveur.

## Ce qui tourne

| Conteneur | Rôle | Limite mémoire |
|---|---|---|
| `server` | Le serveur Beholder, sur le port 8080, accessible uniquement depuis le réseau Docker | 256 Mo |
| `caddy` | TLS sur le 443, transmet tout (y compris le WebSocket `/ws`) au serveur | 128 Mo |

Caddy ne parle que HTTP/1.1 et HTTP/2. HTTP/3 (QUIC sur UDP) est désactivé volontairement, car l'UDP est souvent bridé sur les VPN.

## Comptes

L'inscription est **ouverte** : toute personne qui connaît l'adresse peut créer un compte, mais elle ne voit que les campagnes qu'elle crée ou qu'elle rejoint avec un code. Les tentatives sont limitées (5 inscriptions par heure et 20 connexions par minute par IP, 10 connexions par minute par nom d'utilisateur, 10 tentatives pour rejoindre par minute par utilisateur).
