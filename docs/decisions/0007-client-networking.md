# 0007: Client networking (TLS, HTTP client, connection manager)

## Context

client-core talks to the server over one WebSocket plus a few HTTPS calls (auth, image upload and download). Binary size matters (packages ≤ ~10 MB), links are slow and lossy, and the UI must never wait on the network.

## Decision

- **TLS: the OS stack** via `native-tls` (SChannel on Windows, OpenSSL on Linux), for both `tokio-tungstenite` and `ureq`. No TLS library is compiled into the binary, and the OS keeps certificates and TLS up to date.
- **HTTP: `ureq` 3** (blocking, no hyper), called from `spawn_blocking`. 30 s timeout for auth, 15 min for image transfers (20 MB at ~30 KB/s). `http_status_as_error(false)` so postcard error bodies can be read.
- **Connection manager:** one task per client.
  - States: `Connecting` (first attempt this run), `Online`, `Reconnecting { queued }` after a drop, `Offline { queued }` after 3 consecutive failures (still retrying).
  - Backoff: 0.5 s doubling, capped at 10 s, with equal jitter (half fixed, half random).
  - Liveness: `Ping` every 15 s; no traffic for 35 s means dead. A network change triggers an immediate retry, or a probe ping when online.
  - On every `HelloOk`: store a rotated token, drop campaigns no longer listed, then resend the whole outbox in order (the server deduplicates).
- **Commands are persisted before they are sent.** `Client::submit` writes the outbox, then wakes the manager. A command leaves the outbox only on `Ack` or `Reject`.
- **The session token** is held by the shell (OS keyring) and passed to `Client::start`; rotations come back as `ClientEvent::SessionRotated`.

## Alternatives

- **rustls:** consistent behaviour everywhere, but adds roughly 1–2 MB per binary (ring or aws-lc). Revisit if a target lacks a usable system TLS (Android uses the platform stack too).
- **reqwest:** async, but brings hyper and its stack for three endpoints.
- **Auth over the WebSocket:** would drop the HTTP client, but SPEC keeps auth and images on plain HTTPS, and images need HTTP caching anyway.

## Consequences

The Linux packages depend on the system `libssl`, which every target distro ships. OS network-change hooks are not wired yet; `Client::network_changed` is ready for them.
