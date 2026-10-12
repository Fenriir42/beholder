# Deploying the Beholder server

The server runs on a small Linux VPS (1 GB RAM) as two containers: the server
and Caddy, which terminates TLS on 443. CI builds the image and pushes it to
GitHub Container Registry; the VPS only pulls it.

## First time

1. Point a DNS name (e.g. `dnd.example.org`) at the VPS.
2. Add a small swap file as a safety net:
   ```sh
   sudo fallocate -l 1G /swapfile && sudo chmod 600 /swapfile
   sudo mkswap /swapfile && sudo swapon /swapfile
   echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
   ```
3. Copy `docker-compose.yml` and `Caddyfile` to the VPS, and create `.env` next to them:
   ```sh
   BEHOLDER_DOMAIN=dnd.example.org
   GHCR_OWNER=your-github-user-lowercase
   # BEHOLDER_TAG=v0.1.0   # pin a release instead of latest
   ```
   If the image is private, log in once: `docker login ghcr.io -u <user>` with a
   token that has `read:packages`.
4. Start: `docker compose up -d`. Caddy gets a certificate automatically.

## Update

```sh
docker compose pull && docker compose up -d
```

## Data and backups

Everything lives in the `beholder-data` volume (`beholder.db` and `images/`).
Back it up with the server stopped, or with `sqlite3 beholder.db ".backup b.db"`.

## Memory budget

| Container | Limit | Typical |
|---|---|---|
| server | 256 MB | ~10-20 MB idle, ~150-190 MB during a large image upload |
| caddy | 128 MB | ~30-50 MB |
