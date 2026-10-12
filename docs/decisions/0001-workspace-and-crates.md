# 0001: Workspace layout, crate names and toolchain

## Context

SPEC §2 sets a target layout. CLAUDE.md was written before the app had a name and used a `tavern-` prefix. Phase 1 also needs two places the original map didn't have: types shared by the wasm UI and the native Tauri shell, and end-to-end tests that span server and client.

## Decision

- Crates use the `beholder-` prefix (SPEC wins; CLAUDE.md updated).
- Two crates are added to the SPEC layout:
  - `apps/desktop/ipc` (`beholder-ipc`): serde DTOs for Tauri commands and events. The UI is wasm32 and can't depend on `tauri` or `client-core`, and these types don't belong in the wire protocol (they change with the UI, not with `PROTOCOL_VERSION`).
  - `crates/e2e` (`beholder-e2e`, test-only): the flaky-network test. Keeping it out of `client-core` means `cargo test -p beholder-client-core` never compiles the server.
- Domain events and their pure `apply` live in `beholder-core`, so the server's projection and the client's cache share one state machine.
- The toolchain is pinned in `rust-toolchain.toml` (1.99.0, with the wasm32 target). Bumping it is a deliberate one-line change.
- `data/` is git-ignored except `data/srd-5.2/` and `data/open5e/`. The local `dnd-data` clone contains non-SRD, copyrighted content and is never committed or bundled; SPEC §5.2 records it as a server-setup import into the private library (Phase 3).

## Alternatives

- Putting IPC types in `beholder-core`: mixes UI view models into the domain crate.
- Putting the flaky test in `client-core/tests`: makes client-core's test build pull in axum and the server.

## Consequences

Two more small crates to maintain. The repo map in CLAUDE.md lists them.
