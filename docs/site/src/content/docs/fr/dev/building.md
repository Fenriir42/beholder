---
title: Compiler et lancer
description: Chaîne d'outils, commandes courantes, lancement de l'application, paquets et site de documentation.
sidebar:
  order: 2
---

## Chaîne d'outils

La version de Rust est figée dans `rust-toolchain.toml` (1.99.0, avec la cible `wasm32-unknown-unknown`) ; `rustup` l'installe automatiquement. Sous Linux, le shell de bureau a aussi besoin des paquets de développement WebKitGTK :

```sh
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
```

Deux outils précompilés, installés avec `cargo-binstall` (donc sans rien compiler) :

```sh
curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
cargo binstall -y trunk@0.21.14 tauri-cli@2.12.2
```

## Commandes courantes

L'ordinateur de développement est lent : lancez la commande la plus ciblée qui vérifie votre changement.

```sh
cargo fmt --all
cargo clippy -p beholder-server --all-targets -- -D warnings
cargo test -p beholder-server permissions
cargo check -p beholder-core -p beholder-protocol -p beholder-ui --target wasm32-unknown-unknown
```

Avant un commit qui touche plusieurs crates :

```sh
cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

## Lancer l'application

Terminal 1, le serveur (données dans `./data-dev`) :

```sh
BEHOLDER_DATA_DIR=./data-dev cargo run -p beholder-server
```

Terminal 2, l'application de bureau (elle démarre `trunk serve` pour l'UI sur le port 1420) :

```sh
cd apps/desktop/src-tauri
cargo tauri dev
```

Utilisez `http://127.0.0.1:8080` comme adresse du serveur. Pour essayer à deux joueurs, créez un second compte depuis une autre machine pointant vers votre IP locale, ou déconnectez-vous puis reconnectez-vous.

## Builds de release et paquets

```sh
cd apps/desktop/ui && trunk build --release          # UI seule ; WASM dans dist/
cd apps/desktop/src-tauri && cargo tauri build --bundles deb,rpm   # Linux
cd apps/desktop/src-tauri && cargo tauri build --bundles nsis      # Windows
docker build -f deploy/Dockerfile -t beholder-server .             # image du serveur
```

La CI (`.github/workflows/`) lance fmt, clippy, les tests et la vérification wasm à chaque push, publie la taille du WASM de l'UI et du binaire serveur, pousse l'image du serveur sur GHCR et construit les installateurs sur les tags `v*`.

## Réseau lent

Télécharger les crates via un VPN lent est le goulot d'étranglement habituel. Récupérez-les une fois quand la connexion est bonne, puis travaillez hors ligne :

```sh
export CARGO_NET_RETRY=10 CARGO_HTTP_TIMEOUT=180
cargo fetch
cargo build --offline
```

Un miroir de crates.io comme `rsproxy.cn` peut être configuré dans votre `~/.cargo/config.toml` utilisateur (jamais dans le dépôt).

## Cette documentation

La documentation est un site [Astro Starlight](https://starlight.astro.build/) dans `docs/site`. Il faut Node ≥ 22.12 :

```sh
cd docs/site
npm install
npm run dev      # http://localhost:4321
npm run build    # site statique dans docs/site/dist, avec recherche hors ligne
```
