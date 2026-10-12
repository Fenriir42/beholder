# 0010: Documentation site with Astro Starlight

## Context

The project needs documentation for three audiences (players, the person running the server, developers) in English and French, matching the app. For now it's read locally, not hosted.

## Decision

- An [Astro Starlight](https://starlight.astro.build/) site in `docs/site`, pinned to exact versions (`astro` 7.3.8, `@astrojs/starlight` 0.42.6).
- English at the root, French under `/fr/`, using Starlight's built-in i18n. The sidebar is generated per section (`players`, `admin`, `dev`).
- Image processing is set to the passthrough service, so the native `sharp` binary isn't needed. Telemetry is off in every npm script. Search uses Pagefind, built into the static output, and works offline.
- Theme: the app's accent colour and dark surfaces, system fonts, nothing loaded from external services.
- Node ≥ 22.12 is required by Astro 7. The developer machine's system Node is 20 (end-of-life), so a user-local Node 24 LTS lives in `~/.local/share/node24`, put on `PATH` only when working on the docs. The system Node is untouched.
- Nothing about the docs runs in CI yet, and they aren't shipped with the app.

## Alternatives

- **mdBook:** a single Rust binary, much lighter, in line with "Rust wherever reasonable". But it has no built-in translations, so FR would be a second book, and the look is plainer. The docs aren't shipped, so the Node toolchain doesn't touch the app's size or runtime.
- **Astro 5 on Node 20:** works with the system Node, but pins an older Starlight on an end-of-life runtime.

## Consequences

- Working on the docs needs Node and roughly 200 MB of `node_modules` (git-ignored).
- When the docs are hosted, github.io is unreliable from China, so the plan is to serve the static `dist/` from the VPS through Caddy.
