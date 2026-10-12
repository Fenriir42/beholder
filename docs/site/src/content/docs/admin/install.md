---
title: Install the server
description: Run the Beholder server on a small VPS with Docker and Caddy.
sidebar:
  order: 1
---

One server hosts one group. It's designed for a **1 GB RAM** Linux VPS and uses very little:

| | Measured |
|---|---|
| Server memory when idle | about 6 MB |
| Server memory at peak (re-encoding a 40-megapixel photo) | about 157 MB |
| Server binary | about 6 MB |

The VPS never compiles anything. The image is built by GitHub Actions and the VPS only downloads it.

## What you need

- A Linux VPS with Docker and the Docker Compose plugin.
- A domain name pointing at the VPS (an `A` record, e.g. `dnd.example.org`).
- Ports **80** and **443** open. Caddy uses 80 to obtain the TLS certificate.

## 1. Add a swap file

A small swap file protects the VPS if memory runs short:

```sh
sudo fallocate -l 1G /swapfile && sudo chmod 600 /swapfile
sudo mkswap /swapfile && sudo swapon /swapfile
echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
```

## 2. Copy the deploy files

Copy `deploy/docker-compose.yml` and `deploy/Caddyfile` from the repository to a folder on the VPS (e.g. `~/beholder`), then create `.env` next to them:

```sh
BEHOLDER_DOMAIN=dnd.example.org
GHCR_OWNER=your-github-user-in-lowercase
# BEHOLDER_TAG=0.1.0   # pin a release; default is latest
```

If the image is private on GitHub, log in once with a token that has the `read:packages` scope:

```sh
docker login ghcr.io -u your-github-user
```

## 3. Start

```sh
docker compose up -d
docker compose logs -f
```

Caddy obtains a certificate automatically. Players then enter `https://dnd.example.org` (or just `dnd.example.org`) as the server address.

## What's running

| Container | Role | Memory limit |
|---|---|---|
| `server` | The Beholder server, listening on port 8080 inside the Docker network only | 256 MB |
| `caddy` | TLS on 443, forwards everything (including the `/ws` WebSocket) to the server | 128 MB |

Caddy speaks HTTP/1.1 and HTTP/2 only. HTTP/3 (QUIC over UDP) is off on purpose, because UDP is often throttled on VPNs.

## Accounts

Registration is **open**: anyone who knows the address can create an account, but they only see campaigns they create or join with an invite code. Attempts are rate-limited (5 registrations per hour and 20 logins per minute per IP, 10 logins per minute per username, 10 join attempts per minute per user).
