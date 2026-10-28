---
title: Updates, backups and logs
description: Day-to-day operation of a Beholder server.
sidebar:
  order: 2
---

## Update

```sh
cd ~/beholder
docker compose pull
docker compose up -d
```

Connected players reconnect automatically after a few seconds. Changes they made during the restart are sent once they're back online.

If the new server speaks a newer protocol, older apps show *Please update Beholder* and keep working offline until they're updated.

To stay on a given version, set `BEHOLDER_TAG=0.1.0` in `.env`.

## Where the data lives

Everything is in the `beholder-data` Docker volume:

| Path in the volume | Content |
|---|---|
| `beholder.db` (+ `-wal`, `-shm`) | SQLite database: accounts, sessions, campaigns, event log |
| `images/ab/<hash>-thumb.webp` | Cover thumbnails (~480 px wide) |
| `images/ab/<hash>-display.webp` | Covers at display size (max 1920 px) |

## Back up

The simplest safe backup stops the server for a few seconds:

```sh
docker compose stop server
docker run --rm -v beholder_beholder-data:/data -v "$PWD":/backup alpine \
  tar czf /backup/beholder-$(date +%F).tar.gz -C /data .
docker compose start server
```

The volume name is `<folder>_beholder-data`; check it with `docker volume ls`. To restore, stop the server, extract the archive into the volume the same way, and start it again.

## Import from an external repository

To fill the server library from a repository of JSON files laid out like [`nick-aschenbach/dnd-data`](https://github.com/nick-aschenbach/dnd-data) (one file per category: `spells.json`, `monsters.json`, `items.json`, `classes.json`, `species.json`, `backgrounds.json`), set these in `.env` and run `docker compose up -d`:

```sh
BEHOLDER_IMPORT_REPO=https://github.com/owner/repository
BEHOLDER_IMPORT_REF=HEAD
```

On its next start the server downloads the repository's `.tar.gz` archive over HTTPS (GitHub, GitLab, Gitea and Codeberg style URLs, or a direct link to a `.tar.gz`), reads the JSON files in it one record at a time, converts them like `cargo xtask dnd-data` does, and adds them to the library as one import. Entries already in the library (same type, name, book and rules version) are skipped. It runs once: after a successful import it isn't repeated, even if the variables stay set. If it fails (no network, wrong URL), the log says why and it's tried again at the next start. `BEHOLDER_IMPORT_REF` is a branch or tag; `HEAD` means the default branch on GitHub, and GitLab needs a branch name.

The imported entries belong to the server rather than to a user, so only a DM can edit them. A DM can also undo the whole import from **Recent imports** in the Compendium. The content stays on your server and is never bundled with the app; only import content you're allowed to use.

## Logs

```sh
docker compose logs -f server
```

Set `BEHOLDER_LOG=debug` in `.env` (then `docker compose up -d`) for more detail; `info` is the default.

## Maintenance the server does by itself

Once a day the server:

- keeps the last 1,000 events of each campaign (older history is summarised; apps that were offline for a very long time simply receive the current state),
- forgets command receipts older than 8 days,
- deletes sessions unused for 30 days.
