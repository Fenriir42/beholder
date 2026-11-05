# 0027: Docs served by the server

## Context

The documentation site (Astro Starlight, EN and FR) was local only. The user wants the deployed image to serve the docs as well as the server, so the group can read the player and admin pages at the same address, including from China, where external doc hosts may be slow or blocked.

## Decision

- **Build:** the Dockerfile gains a Node stage (`node:22.12.0-alpine3.21`, `npm ci` + `astro build`). The static site, about 2.8 MB, is copied to `/docs` in the `scratch` image, and `BEHOLDER_DOCS_DIR=/docs` is set. The server binary doesn't change size.
- **Base path:** Astro uses `base: '/docs'`, and absolute links in the pages carry the prefix.
- **Serving:** a small handler in `crates/server/src/docs.rs` serves `/docs/*` from that directory.
  - It accepts only plain path segments, so a `..` request can never leave the docs folder.
  - It maps `/` to `index.html`, redirects folders without the trailing slash, and answers misses with the site's `404.html`.
  - Hashed `_astro/` assets are cached for a year; pages are revalidated.
  - `/` redirects to `/docs/`.
  - Without `BEHOLDER_DOCS_DIR`, these routes return 404.
- No new crate: a content-type table and `tokio::fs` cover static pages.

## Alternatives

- **`tower-http`'s `ServeDir`:** range requests and precompressed files out of the box, at the cost of a new dependency tree. Neither feature matters for 41 small pages.
- **A separate docs container:** a clean split, but another Deployment, quota and Traefik route in the sandbox.
- **Hosting on GitHub Pages:** free, but another place to maintain and not reliably reachable from China.

## Consequences

- The image build takes longer by the docs build (about a minute, cached when the docs don't change).
- The docs are public on the server's address. They contain nothing private.
- Local docs development opens at `/docs/`.
