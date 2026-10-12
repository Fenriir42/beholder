---
title: Building and running
description: Toolchain, everyday commands, running the app, packaging and the docs site.
sidebar:
  order: 2
---

## Toolchain

The Rust version is pinned in `rust-toolchain.toml` (1.99.0, with the `wasm32-unknown-unknown` target); `rustup` installs it automatically. On Linux the desktop shell also needs the WebKitGTK development packages:

```sh
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
```

Two prebuilt tools, installed with `cargo-binstall` (so nothing is compiled):

```sh
curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
cargo binstall -y trunk@0.21.14 tauri-cli@2.12.2
```

## Everyday commands

The development laptop is slow, so run the narrowest command that verifies a change:

```sh
cargo fmt --all
cargo clippy -p beholder-server --all-targets -- -D warnings
cargo test -p beholder-server permissions
cargo check -p beholder-core -p beholder-protocol -p beholder-ui --target wasm32-unknown-unknown
```

Before a commit that touches several crates:

```sh
cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

## Run the app

Terminal 1, the server (data in `./data-dev`):

```sh
BEHOLDER_DATA_DIR=./data-dev cargo run -p beholder-server
```

Terminal 2, the desktop app (it starts `trunk serve` for the UI on port 1420):

```sh
cd apps/desktop/src-tauri
cargo tauri dev
```

Use `http://127.0.0.1:8080` as the server address. To try two players, register a second account on another machine pointing at your LAN IP, or log out and in again.

## Release builds and packages

```sh
cd apps/desktop/ui && trunk build --release          # UI only; WASM in dist/
cd apps/desktop/src-tauri && cargo tauri build --bundles deb,rpm   # Linux
cd apps/desktop/src-tauri && cargo tauri build --bundles nsis      # Windows
docker build -f deploy/Dockerfile -t beholder-server .             # server image
```

CI (`.github/workflows/`) runs fmt, clippy, tests and the wasm check on every push, reports the UI WASM and server binary sizes, publishes the server image to GHCR, and builds the installers on `v*` tags.

## Slow networks

Downloading crates over a slow VPN is the usual bottleneck. Fetch once while the connection is good, then work offline:

```sh
export CARGO_NET_RETRY=10 CARGO_HTTP_TIMEOUT=180
cargo fetch
cargo build --offline
```

A crates.io mirror such as `rsproxy.cn` can be set in your user-level `~/.cargo/config.toml` (never in the repository).

## This documentation

The docs are an [Astro Starlight](https://starlight.astro.build/) site in `docs/site`. They need Node ≥ 22.12:

```sh
cd docs/site
npm install
npm run dev      # http://localhost:4321
npm run build    # static site in docs/site/dist, with offline search
```
