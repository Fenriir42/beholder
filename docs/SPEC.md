# Beholder: project spec

App name: **Beholder**. Crates and binaries use the `beholder-` prefix (e.g. `beholder-server`).

A companion app for a small group playing D&D 5e on the **2024 rules (SRD 5.2)**. It has a Linux server, a desktop client for Linux and Windows, and an Android client later. It is used by one friend group, so it doesn't need to scale to thousands of users. It does need to be fast, light and solid on bad networks.

This file is the reference for the whole project. Each implementation phase has its own prompt. Don't build features from later phases early, but design so they fit without rewrites.

---

## 1. Principles (in priority order)

1. **Optimisation.** Low CPU, RAM, disk and network use. The main developer uses a weak laptop on an unstable VPN from China. Every dependency must justify its size and compile time. Prefer simple data structures and avoid needless allocations and abstraction layers.
2. **Maintainability.** Small focused crates, clear module boundaries, typed errors, and tests for every rule calculation and every permission check. Readable code beats clever code.
3. **Rust everywhere it is reasonable.** That covers the server, the client core and the UI (Leptos compiled to WASM). HTML/CSS stay only as the rendering layer.
4. **The server is the authority.** Clients cache and queue, but the server decides. **Hidden data is never sent to a client that may not see it.** Filtering happens server-side, never by hiding things in the UI.
5. **No runtime dependency on third-party CDNs or Google services.** Fonts and assets are bundled, because those services are blocked or slow in China. Discord is reached only for the optional Discord login.

---

## 2. Platforms and stack

| Part | Choice |
|---|---|
| Server | Rust, `tokio` + `axum`, SQLite (via `rusqlite`, bundled, migrations in repo; see ADR-0002), deployed as a Docker image on a Linux VPS with **1 GB RAM**, behind Caddy (TLS on 443) |
| Desktop client | **Tauri 2** for Linux and Windows. It uses the system webview: WebKitGTK on Linux, WebView2 on Windows |
| UI | **Leptos** in CSR mode, compiled to WASM, built with Trunk |
| Client core | Plain Rust crate with no Tauri dependency, used by the Tauri backend: connection manager, outbox, local cache (SQLite via `rusqlite`, bundled) |
| Android (later) | The same Tauri 2 app, with one APK per ABI (arm64-v8a, armeabi-v7a, x86_64) and no universal fat APK |
| Discord bot (later) | Rust crate in the same workspace, reusing `core` and `protocol` |

Always check the latest stable versions of crates and tools instead of relying on memory.

### Repository layout (target)

```
Cargo.toml                 # workspace
crates/
  core/                    # domain types, rules calculations, dice parser. No IO. Builds for native + wasm32
  protocol/                # wire messages, protocol version, codec (postcard + zstd)
  server/                  # axum server binary
  client-core/             # connection manager, outbox, local cache, sync; no Tauri dependency (testable headless)
apps/desktop/
  src-tauri/               # Tauri shell: commands + events bridging UI <-> client-core
  ui/                      # Leptos CSR app (wasm32)
data/srd-5.2/              # vendored SRD JSON (pinned commit) + attribution text
data/open5e/               # vendored open-licensed content (pinned commit) + licence texts
deploy/                    # Dockerfile, Caddyfile, docker-compose.yml
docs/SPEC.md               # this file
docs/decisions/            # short ADRs (one file per significant decision)
CLAUDE.md                  # conventions for Claude Code
```

The UI never opens network connections itself. It talks only to the Tauri backend (`invoke` + events), and the backend owns the connection and the cache. A network hiccup can therefore never freeze the UI.

### Build profiles

- **Client and WASM:** optimise for size. That means `opt-level = "z"` or `"s"` (measure both), `lto = "fat"`, `codegen-units = 1`, `strip = true`, `panic = "abort"`, and `wasm-opt -Oz` on the WASM output.
- **Server:** optimise for speed (`opt-level = 3`, LTO). Keep unwinding so that one panicking task can't kill the server.
- **Dev builds:** keep them fast on a weak laptop. Use `opt-level = 0` for our crates and 1–2 for dependencies, split debuginfo, and an optional `mold` linker on Linux.

### Packaging

- **Windows:** NSIS installer. WebView2 is installed only when missing (use the download bootstrapper, not the embedded runtime).
- **Linux:** `.deb`, `.rpm` and an AUR `PKGBUILD`. **No AppImage**, because it bundles WebKit and becomes huge.
- **Server:** a multi-stage Docker build producing a minimal runtime image (static musl or distroless). It ships with a `docker-compose.yml` that includes Caddy.
- **CI:** GitHub Actions runs fmt, clippy (`-D warnings`) and tests on every push. A release matrix builds the Linux and Windows packages plus the server image. CI prints artifact sizes so regressions are visible.

### VPS constraints (1 GB RAM for everything)

- **Never compile on the VPS.** CI builds the server image and pushes it to GitHub Container Registry. The VPS only pulls the image and restarts.
- Set **memory limits** in `docker-compose.yml` (roughly 256 MB for the server and 128 MB for Caddy) and keep a small swap file as a safety net.
- **Image uploads:** read the dimensions from the header first, and reject anything over a size or pixel limit (e.g. 20 MB or 40 MP) before decoding. Encode one image at a time, behind a semaphore, because a large photo decoded in full takes 100 MB or more.
- **Compendium bundles** are built ahead of time and served as precompressed static files. They are never assembled in RAM per request.
- **SQLite:** use WAL mode with a bounded page cache.
- Per-connection channels and buffers are bounded, so a slow client can't make memory grow.

### Size and performance targets (watch them, don't obsess)

- UI WASM ≤ ~1.5 MB after `wasm-opt`, before compression.
- Windows installer and Linux packages ≤ ~10 MB each.
- Cold start to the campaign library is served from the local cache in < 1 s, without waiting for the network.
- An idle connection sends only heartbeats, a few hundred bytes per minute.
- The server idles under ~50 MB RAM and stays under ~200 MB at peak, image encoding included.

---

## 3. Networking and sync

This is a core feature: the app must stay smooth on an unstable VPN.

- **Transport:** one WebSocket over TLS (`wss://host/ws`, port 443, behind Caddy). Plain HTTPS is used only for auth, image upload and download, and the compendium bundle download. Don't use QUIC or raw UDP, since they're often throttled over VPNs and in China.
- **Encoding:** binary frames only. Messages are `postcard`-serialized enums from the `protocol` crate. Payloads above a threshold (~1 KB) are zstd-compressed, marked by a flag byte. WebSocket `permessage-deflate` is off.
- **Handshake:** the client sends `Hello { protocol_version, session_token, subscriptions: [(campaign_id, last_seq)] }`. The server answers with only what the client missed, or with a snapshot when it is too far behind. Incompatible protocol versions get a clear "please update" error.
- **Event log:** each campaign has a monotonically increasing `seq`. All state changes are events. Clients apply them to their local cache. Old events can be compacted into snapshots on the server.
- **Per-recipient filtering:** before sending, the server computes what each user may see. When visibility changes (a note is revealed, a campaign setting changes), the server sends upserts to users who gain access and removals to users who lose it. Gaps in `seq` for a given client are normal.
- **Commands and outbox:** every user action is a `Command { id: UUIDv7, campaign_id, kind }`. The server answers `Ack { id, seq }` or `Reject { id, reason }`. The client stores commands in a persistent outbox (SQLite) and resends them after a reconnect. The server deduplicates by `id`, keeping recent IDs for at least 24 h, so a command is never applied twice. The UI updates optimistically and rolls back on `Reject`.
- **Liveness:** an application-level ping every ~15 s; a connection with no traffic for ~35 s counts as dead. Reconnects use exponential backoff with jitter, starting at ~0.5 s and capped at ~10 s, and retry immediately when the OS reports a network change, where supported.
- **Connection states** shown in the UI: `Online`, `Connecting`, `Reconnecting (n queued)`, `Offline (cached)`. When the app is not online, a non-blocking banner explains that changes are queued.
- **Coalescing:** rapid edits such as typing or HP clicks are debounced (~500 ms–1 s) into one command.
- **Text conflicts (v1):** whole-document replace with a version check. If there is a conflict, keep both versions as a conflict copy and tell the user. No CRDT in v1.
- **Images:** on upload the server re-encodes to **lossy WebP**: a thumbnail (~480 px wide) plus a display size (max ~1920 px). Images are content-addressed by hash and served with `Cache-Control: immutable`. Clients cache them on disk by hash and download full size only on demand.
- **Compendium:** each bundled source (the SRD, and each open-licensed source separately) is a prebuilt, versioned bundle identified by a content hash. Clients download only the sources their campaigns enable, once, compressed, and store them locally with FTS5 for search. The server library is also split by source:
  - Each source is synced as a compressed, content-hashed bundle that the server rebuilds to disk after imports, with small edits arriving as normal events.
  - A campaign enables or disables library sources, so a client never downloads sources its campaigns don't use.
- **Presence:** members are shown online or offline, and that's the only always-on traffic.

---

## 4. Accounts, campaigns and permissions

- **Login:** username + password (Argon2id) **and** Discord OAuth2, which can be linked to the same account. Discord login is a later phase, but the account model supports it from the start. Session tokens are opaque, stored server-side and rotated, and kept in the OS keyring on the client. Password login must always work, because Discord is unreachable from China without a VPN.
- **Server address:** the client asks for the server URL on first launch and remembers it.
- **Multiple campaigns:** a user can be DM in one campaign and a player in another. The DM creates a campaign and players join with an **invite code**, which the DM can regenerate to stop new joins.
- **Campaign settings** (changed by the DM, visible read-only to every player, so nothing is hidden from them):
  - `dm_can_read_private_notes`: bool, default false.
  - `combat_visibility`: `exact` | `vague` | `hidden` (what players see of monsters and NPCs in the tracker).
  - Cover image and title.
- Every permission rule lives in one place on the server, as a pure function that is unit-tested.

---

## 5. Features (v1)

### 5.1 Character sheets (assisted)

- Each user can have several characters per campaign, with exactly one **active**. The active character is the one used by the party view, the combat tracker and rolls. Characters belong to the user's account.
- **Computed by the app:** ability modifiers, proficiency bonus, saves, skills (including expertise), passive scores, initiative, spell save DC and spell attack, spell slots from class tables (including the multiclass table), and AC from a selectable formula (armor + Dex cap, unarmored defense variants, shield, manual bonuses).
- **Chosen by the player:** species, class/subclass, background, feats, features. The app links them from the compendium but does not try to apply every rule automatically (no full rules engine).
- **Tracked:** HP (current/max/temp), hit dice, death saves, conditions, exhaustion (2024 rule: −2 per level to d20 tests), generic resource counters (sorcery points, etc.), spells known/prepared, inventory, currency, and a free-text notes field.
- All calculations are pure functions in `core`, with unit tests covering levels 1–20.

### 5.2 Compendium

The goal is a full compendium without typing everything by hand. The app bundles only content that may legally be redistributed, and makes it fast to bring in content the group owns. Every entry is tagged with its source, and the UI always shows it.

1. **SRD 5.2** (official core rules, always on)
   - It comes from `5e-bits/5e-database`, folder `src/2024/en/` (now archived; the same data lives in `5e-bits/5e-srd-api` under `packages/5e-database/`, see ADR-0012): 12 classes, 12 subclasses, 9 species, 339 spells, 341 monsters, plus feats, backgrounds, items and features.
   - Vendor it at a **pinned commit**, convert it into our own types with a small `xtask`, and validate the result. The repo's code is MIT and the content is CC-BY-4.0.
   - **Required attribution** (Credits screen and docs): *"This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode."*
2. **Open-licensed third-party content** (optional, the DM toggles each source per campaign)
   - It comes from `open5e/open5e-api`, folder `data/v2/`, vendored at a pinned commit. Everything there is OGL 1.0a or CC-BY-4.0, so it can be bundled.
   - That's roughly 2,900 monsters, 1,100 spells, 1,060 magic items, 40 species and 117 classes and subclasses. Most of it comes from Kobold Press (Tome of Beasts 1–3, Creature Codex, Deep Magic, Tome of Heroes, Vault of Magic, Black Flag SRD) and EN Publishing (Level Up A5E).
   - Most of it is written for 2014-era 5e, so record the rules version per source. Monsters, items and most spells work as they are.
   - Level Up (A5E) player options use different mechanics, so they are off by default.
   - The OGL requires its full licence text and each source's Section 15 notice, and CC-BY requires attribution. Generate a **Credits** screen from source metadata.
3. **Server library** (homebrew, and content from books the group owns)
   - There is one shared library per server, since a server belongs to one group. An entry is added once and is available in every campaign. A campaign can hide library entries it doesn't use.
   - **Paste import, single or bulk:** the user pastes text from a book or D&D Beyond page they own. It can be one entry (a spell, feat, species, subclass, item or monster stat block) or a whole section with many entries, such as every spell in a chapter.
     - The parser splits the text into entries and fills structured fields, then shows a review list. From there the user can edit any entry, accept all, or accept some.
     - Duplicates (same name, source and rules version) are flagged, with the choice to skip or replace.
     - These texts follow a regular layout. The parser lives in `core` and is tested by rendering SRD entries to text and parsing them back, both one at a time and concatenated. Anything it can't parse goes into a free-text field instead of failing.
     - Every library entry records its **source book**, chosen from the built-in list in Appendix A or typed freely, and its **rules version** (2014 or 2024). The source and rules version are pre-filled once per paste session, not asked for each entry.
   - **External repository import at server setup:** when the server is installed, a setup prompt asks whether to import data from an external repository. If yes, the admin enters a git repo URL, which is assumed to be structured like `nick-aschenbach/dnd-data` (one JSON array per category, entries with `name`, `description`, `properties`, `publisher`, `book`). Entries go into the server library through the same streaming bulk-import path (with an import batch that can be undone), tagged with their `book`/`publisher` as source. This content is never bundled with the app.
   - **In-app editor** with forms per type: species, class, subclass, spell, feat, background, item, monster, generic feature.
   - **Import/export** of JSON and YAML files with a documented, versioned schema. Export always uses our schema.
   - **Bulk JSON import of external formats:** the user picks a local JSON file of any shape, and a **mapping profile** says how to read it.
     - The profile defines where the array of entries is, and which keys hold the name, entry type, description, source book, publisher and the structured properties (including nested `properties` maps).
     - Profiles are saved, reusable and exportable. The app suggests a mapping from the first entries of the file, and the user confirms or edits it.
     - Recognised fields map to structured data (level, school, CR, AC, HP and so on). Everything else is kept as free text. A partly mapped entry is never rejected.
     - Imports are built for **tens of thousands of entries and files of ~100 MB**:
       - The desktop app does the parsing and normalising, with a streaming parser.
       - Entries are uploaded in compressed batches (~500 at a time) through the outbox, so an import resumes after a disconnect.
       - The server inserts each batch in one transaction.
       - Neither side ever holds the whole file in memory (the 1 GB VPS limit applies).
     - Large imports end with a **summary review** instead of per-entry review: counts per type and source, warnings, and duplicates (same name, source and rules version) with skip or replace for all.
     - Each import is recorded as an import batch, so it can be **undone** in one action.
   - Library content stays on the group's private server. **Only SRD and open-licensed content is bundled.** The app never ships, scrapes or downloads other content.
- Search is fast, local and works offline (FTS5), with filters by type, level, school, class and **source**.

### 5.3 Notes and handouts

- Notes are Markdown with optional images (handouts are notes with images).
- **Every note has an audience:** `private` (author only), `party` (all members), or `users([...])` (specific members).
- A DM's private note is "DM only". **Reveal** changes the audience in one action.
- If `dm_can_read_private_notes` is on, the DM also receives players' private notes, and players can see that this setting is on.
- Navigation: the **Notes** section in the sidebar is the full editor and list. The **Campaign notes** tab on the campaign home only previews notes visible to the whole party.
- Live updates: a revealed handout appears for everyone immediately.

### 5.4 Combat tracker

- The DM creates an encounter and adds active PCs plus monsters (from the compendium or quick-added by name/HP/AC). Hidden combatants are allowed and stay invisible to players until revealed.
- Initiative (rolled or entered), turn and round counter, HP and temp HP, conditions with optional duration in rounds, and a concentration flag.
- **Player view** depends on `combat_visibility`:
  - `exact`: everything.
  - `vague`: turn order and conditions, with monster health shown as **Healthy** (> 50 %), **Bloodied** (≤ 50 %) or **Near death** (≤ 25 %).
  - `hidden`: players don't see the tracker.
  - PC HP is always exact.
- The server filters all of this per recipient (see principle 4).

### 5.5 Dice

- A dice-notation parser in `core`: `NdM`, `+/-` modifiers, keep highest/lowest (`2d20kh1`), advantage/disadvantage shortcuts.
- Clicking a skill, save, attack or spell on the sheet builds the expression with the computed modifiers.
- Digital rolls are made **on the server** (one authoritative path, which the Discord bot will reuse later), with an optimistic "rolling…" state in the UI.
- **Shared roll log** per campaign, updated live.
- **Secret DM rolls** are visible only to the DM.
- **Physical dice entry:** a player types the result of real dice, and the log marks it as physical.
- Rolls are events with a structured payload (who, expression, individual dice, total, context such as "Dexterity save"), so a future Discord bot can post and receive them.

### 5.6 Internationalisation

- UI in **English and French**, switchable at runtime, using Fluent (`.ftl`) files. SRD content stays in English.

---

## 6. Design

The style is **modern dark**, clean like a launcher app (CurseForge-like library), with the fantasy touch coming from the display font and the accent colour.

### Tokens

| Token | Value |
|---|---|
| bg | `#121318` |
| bg-chrome (top bar, sidebar) | `#16171d` |
| surface | `#1a1c23` |
| surface-raised | `#1d1f27` |
| border | `#2c3039` |
| border-strong | `#343844` |
| text | `#ecebe6` |
| text-2 | `#c9cbd2` |
| text-muted | `#a3a6b0` |
| placeholder | `#8a8e99` |
| accent (default, user-selectable) | `#e3a94f` (alternatives: `#7aa7ff`, `#b48cff`, `#5cc8a8`) |
| on-accent | `#17130a` |
| status online / warning / error | `#4cc3a5` / `#e3a94f` / `#e06c6c` |
| audience chip: party | bg `#1d302b`, text `#93e2c9` |
| audience chip: shared | bg `#1e2639`, text `#adc6ff` |
| audience chip: private | bg `#272932`, text `#c9cbd2` |
| audience chip: DM only | bg `#341f2a`, text `#f4b9cb` |

- **Fonts:** Cinzel (600/700) for display headings only, and Manrope (400–700) for everything else. Bundle both as subset `woff2` (Latin + Latin-ext for French) and never load them from Google Fonts.
- **Radii:** 10 px for controls, 12 px for cards and lists, 14 px for character cards.
- **Controls and icons:** touch targets ≥ 44 px. Icons are inline SVG with a 1.8 stroke and round caps.
- **Accessibility:** text contrast ≥ 4.5:1. Status is never conveyed by colour alone (always a label or icon too).

### Screens

- **First launch:** server URL, then login/register (Discord button shown but disabled until that phase).
- **Library (main page after login):**
  - Top bar: logo and name, a connection pill (dot + label), a language switch and an account button.
  - Heading "Your campaigns", a search field, a "Join with code" button and a "New campaign" button (accent).
  - A plain card grid (`auto-fill`, min ~240 px) where each card shows **only a 4:3 cover and the title**. The last card is a dashed "Join or create a campaign".
  - Clicking a card opens the campaign.
- **Campaign view:**
  - Left sidebar: "All campaigns" back link, mini cover + campaign name, nav (Home, Character sheet, Notes, Compendium, Combat, Dice log), and the connection pill at the bottom. On Android the sidebar becomes a bottom bar.
  - On desktop, sections can be shown side by side in **split panels** (e.g. sheet + dice log).
- **Campaign home:**
  - A 150 px cover banner, the title in Cinzel, and a meta line ("DM: X · N members" or "You are the DM · N members").
  - Tabs:
    - **Characters:** your characters, with the active one highlighted, "Open sheet", "Set active", "New character" and "Import".
    - **Campaign notes:** a preview of party-visible notes with audience chips.
    - **Party:** members with presence and the DM badge.
    - **Settings:** editable by the DM, shown read-only as **Campaign rules** to players.
- **Offline / reconnecting banner:** shown under the top bar, non-blocking.

---

## 7. Quality and conventions

- `cargo fmt`, and `clippy -D warnings` in CI. No `unwrap()`/`expect()` outside tests and startup.
- Errors use `thiserror` in libraries and `anyhow` only in binaries.
- **Tests:**
  - Rules calculations.
  - Dice parser (property tests welcome).
  - Protocol round-trips.
  - Permission filtering: **a player must never receive DM-only data**, tested at the server's send boundary.
  - A **flaky-network integration test**: a small TCP proxy that adds latency and randomly drops connections must lead to no lost and no duplicated commands, with the client converging to server state.
- Write a short ADR in `docs/decisions/` for each significant choice (crate picks, protocol format, sync strategy).
- Keep the dependency list lean. Justify each new crate in the PR or commit message.

---

## 8. Phases

1. **Foundation:** workspace, CI, protocol, server core (auth with password, campaigns, invites, settings, event log, sync, presence, covers), client-core (connection, outbox, cache), desktop shell (login, library, campaign home shell, i18n, design tokens), packaging, Docker.
2. **Notes and handouts** with audiences, reveal, and the DM-read setting.
3. **Compendium:** SRD 5.2 and Open5e sources, local search, server library with paste import, external repository import at server setup, editor, JSON/YAML import/export, bulk JSON import with mapping profiles, Credits screen.
4. **Character sheets** (assisted).
5. **Dice:** roll from sheet, log, secret rolls, physical entry.
6. **Combat tracker.**
7. **Discord login** and account linking.
8. **Later:**
   - Android app.
   - Homebrew Discord bot (rolls with sheet modifiers, mirrored between Discord and the app).
   - Other game systems (e.g. Imperium Maledictum, Call of Cthulhu). Every compendium source is already tagged with its game system, and a campaign only uses sources of its own system.
   - Homebrew game systems, defined in the app within what it can model (stats, rolls, entry types).
   - Scripting for custom behaviour per campaign.
   - A modding and add-on system for the whole app, so anyone can extend it.
