---
title: Run a server locally
description: Run the server on your own machine, for testing or for a LAN game.
sidebar:
  order: 3
---

You can run the server straight from the source code, without Docker. See [Building and running](/dev/building/) for the toolchain.

```sh
BEHOLDER_DATA_DIR=./data-dev cargo run -p beholder-server
```

It listens on `0.0.0.0:8080`. In the app, use `http://127.0.0.1:8080` as the server address (or `http://<your-LAN-IP>:8080` from another computer on the same network).

| Variable | Default | Meaning |
|---|---|---|
| `BEHOLDER_BIND` | `0.0.0.0:8080` | Address and port to listen on |
| `BEHOLDER_DATA_DIR` | `data-dev` | Folder for the database and images |
| `BEHOLDER_LOG` | `info` | `error`, `warn`, `info`, `debug` or `trace` |

:::caution
Without Caddy there's no TLS: passwords travel unencrypted. Use plain `http://` only on your own machine or a trusted LAN.
:::

To start from scratch, stop the server and delete the data folder.
