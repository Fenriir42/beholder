# 0009: Server deployment and desktop packaging

## Context

SPEC §2: the VPS has 1 GB RAM and must never compile. The server ships as a Docker image behind Caddy; desktop packages are NSIS on Windows and .deb/.rpm on Linux, with no AppImage.

## Decision

- **Image:** multi-stage build on `rust:1.99.0-alpine3.22`, producing a fully static musl binary (`--profile server`: opt-level 3, fat LTO, unwinding) in a `scratch` image. It runs as UID 10001 with data in `/data`. No shell, no libc, nothing else to patch.
- **musl allocator:** slower than glibc's, but it returns memory to the OS readily, which suits a memory-capped container. At this load allocation speed doesn't matter.
- **CI publishes, the VPS pulls:** `docker.yml` builds with BuildKit and GitHub Actions cache and pushes `latest`, the semver tag and the SHA tag to GHCR.
- **Compose:** server limited to 256 MB (plus a little swap), Caddy to 128 MB with `GOMEMLIMIT=96MiB`. Caddy serves HTTP/1.1 and HTTP/2 only (no QUIC, per SPEC §3), caps bodies at 21 MB, and proxies everything, including the `/ws` upgrade, to the server. A 1 GB swap file is the documented safety net.
- **Desktop bundles:** `release.yml` matrix with Ubuntu 22.04 (`deb,rpm`, older glibc for wider compatibility) and Windows 2022 (`nsis`, WebView2 via the download bootstrapper, per-user install). Prebuilt `trunk` and `tauri-cli` come from `taiki-e/install-action`. Artifacts go to a draft GitHub release, with sizes in the job summary.

## Alternatives

- **distroless/cc + glibc:** dynamic linking, a bigger image, and glibc arenas that need tuning (`MALLOC_ARENA_MAX`) to keep RSS down.
- **tauri-action:** less explicit than three commands, and harder to report sizes from.
- **AUR PKGBUILD:** SPEC lists it; deferred until there's a first release to point it at.

## Consequences

Image builds are cached per layer but rebuild all Rust code when the lockfile changes. The server doesn't make outbound HTTPS calls in Phase 1, so the image has no CA bundle; Discord login (Phase 7) will need one added.
