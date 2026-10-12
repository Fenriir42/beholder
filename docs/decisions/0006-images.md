# 0006: Image uploads and storage

## Context

SPEC §2–3: covers are re-encoded to lossy WebP (thumbnail ~480 px wide, display ≤ 1920 px), content-addressed and served immutable. The server container has 256 MB; a big photo decoded in full takes 100 MB or more.

## Decision

- `POST /api/images` with raw bytes and a Bearer token. At most 2 bodies are buffered at once (20 MB cap each via `to_bytes`).
- The hash is SHA-256 of the uploaded bytes. A known hash returns immediately without decoding.
- Before decoding: the format must be JPEG, PNG or WebP, and the header's dimensions must be ≤ 40 MP. Decoding then runs under `image::Limits` with a 170 MB allocation cap, one at a time (1-permit semaphore) on the blocking pool.
- Resizing: `thumbnail` (area averaging, fast and low-memory) to the display size, which drops the full decode; then `Triangle` resize to the 480 px thumbnail. No upscaling.
- Encoding: lossy WebP at quality 80 through the `webp` crate (safe wrapper over libwebp). Alpha is kept when present.
- Files: `images/<first byte hex>/<hash>-{thumb,display}.webp`, written to a temp file and renamed. `GET /img/{hash}/{thumb|display}.webp` sends `Cache-Control: public, max-age=31536000, immutable`.
- Image URLs are unguessable (256-bit hash) and not otherwise access-controlled.

## Alternatives

- **`image`'s own WebP encoder:** lossless only, so files would be several times larger.
- **Calling libwebp through `libwebp-sys` ourselves:** needs `unsafe` in our code, which CLAUDE.md forbids.
- **Client-side downscaling before upload:** would save a lot of VPN upload time, but adds `image` to the desktop binary. Deferred.

## Consequences

Measured peak server RSS while re-encoding a 39.9 MP JPEG: 157 MB (idle: 6 MB). When handouts with DM-only images arrive (Phase 2), revisit whether hash-only access is enough.
