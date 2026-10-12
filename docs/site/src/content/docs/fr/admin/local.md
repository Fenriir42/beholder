---
title: Lancer un serveur en local
description: Faire tourner le serveur sur sa propre machine, pour tester ou pour une partie en réseau local.
sidebar:
  order: 3
---

Vous pouvez lancer le serveur directement depuis le code source, sans Docker. Voir [Compiler et lancer](/fr/dev/building/) pour la chaîne d'outils.

```sh
BEHOLDER_DATA_DIR=./data-dev cargo run -p beholder-server
```

Il écoute sur `0.0.0.0:8080`. Dans l'application, utilisez `http://127.0.0.1:8080` comme adresse du serveur (ou `http://<votre-IP-locale>:8080` depuis un autre ordinateur du même réseau).

| Variable | Défaut | Rôle |
|---|---|---|
| `BEHOLDER_BIND` | `0.0.0.0:8080` | Adresse et port d'écoute |
| `BEHOLDER_DATA_DIR` | `data-dev` | Dossier de la base et des images |
| `BEHOLDER_LOG` | `info` | `error`, `warn`, `info`, `debug` ou `trace` |

:::caution
Sans Caddy, il n'y a pas de TLS : les mots de passe circulent en clair. N'utilisez `http://` que sur votre propre machine ou un réseau local de confiance.
:::

Pour repartir de zéro, arrêtez le serveur et supprimez le dossier de données.
