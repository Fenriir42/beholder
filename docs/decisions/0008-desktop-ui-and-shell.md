# 0008: Desktop UI and Tauri shell

## Context

SPEC §2, §5.6 and §6: Tauri 2 with a Leptos CSR UI, EN/FR with Fluent, design tokens, bundled fonts, UI WASM ≤ ~1.5 MB, and a UI that never touches the network.

## Decision

- **No router.** A desktop window has no URL bar; an enum `Screen` signal switches screens. `leptos_router` would add WASM size for nothing.
- **IPC without a wrapper crate.** `withGlobalTauri` exposes `window.__TAURI__`; the UI binds `core.invoke` and `event.listen` with `wasm-bindgen` and converts with `serde-wasm-bindgen` (JSON-compatible mode). DTOs live in `beholder-ipc`, shared with the shell. Commands use `rename_all = "snake_case"`.
- **Cover uploads bypass WASM memory.** The picked `File` is read into a JS `ArrayBuffer` and passed to `invoke` as a raw body; the shell reads it from `tauri::ipc::Request`. A 20 MB photo never lands in WASM linear memory (which never shrinks).
- **Images through a custom protocol.** The shell registers `img://` (`http://img.localhost` on Windows): it serves covers from a disk cache keyed by content hash and downloads misses through client-core. Responses are immutable-cacheable.
- **Fluent directly.** `fluent-bundle` with both `.ftl` files embedded via `include_str!`; a `RwSignal<Lang>` makes every string reactive, so switching language is instant. Unicode isolation marks are off.
- **Coalesced refreshes.** `view-changed` events trigger one `view` call at a time; events arriving meanwhile cause exactly one more.
- **Session lifecycle in the shell.** Token in the OS keyring via `keyring-core` (Secret Service on Linux with OpenSSL crypto, which native-tls already links; Credential Manager on Windows). With no keyring, the session lasts until the app closes. On startup with a remembered server, user and token, the shell opens that user's cache and starts the client before the window asks, so the library renders from cache immediately.
- **Fonts:** variable Cinzel and Manrope woff2, Latin and Latin-ext subsets from fontsource 5.3.0 (OFL, licences alongside), about 80 KB in total, loaded per `unicode-range`.
- **Security:** a strict CSP (`self`, `wasm-unsafe-eval`, the `img` scheme, IPC). Capabilities grant only `core:event:allow-listen` / `allow-unlisten`.

## Alternatives

- **tauri-sys / leptos_router / leptos_i18n:** convenience at a size cost, and leptos_i18n isn't Fluent.
- **tauri-plugin-dialog for the file picker:** a native dialog, but another plugin (and GTK dialog code) for one input; `<input type=file>` works in both webviews.
- **Serving images via `asset:` and `convertFileSrc`:** needs the `protocol-asset` feature and filesystem scopes; the custom scheme is narrower.

## Consequences

Some UI strings ("Coming soon" nav items) stand in for later phases. When Android arrives, the sidebar becomes a bottom bar, and the `img` base URL already differs per platform.
