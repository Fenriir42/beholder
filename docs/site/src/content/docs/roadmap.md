---
title: Roadmap
description: What Beholder can do today, what's being built, and what's planned.
---

Beholder is built in phases (see `docs/SPEC.md`). This page is updated whenever a part lands.

## Done

### Phase 1: Foundation
- Rust server (axum + SQLite) and desktop app (Tauri 2 + Leptos) for Linux and Windows.
- Accounts with username and password, sessions kept in the OS keyring.
- Campaigns: create, join with an invite code, regenerate the code, cover image, title.
- Campaign rules: whether the DM can read players' private notes, and what players see in combat.
- Sync built for bad connections: changes saved on your computer first and sent when possible, never applied twice; works offline from the local cache; connection pill and banner.
- English and French interface, switchable at any time.
- Server image, Docker Compose with Caddy, installers built by CI.

### Phase 2: Notes and handouts
- Markdown notes with images.
- Audiences: private, DM only, whole party, or chosen members. The server never sends a note to someone outside its audience.
- One-click **Reveal** to the party; a revealed handout appears for everyone at once.
- Collaborative editing for shared notes, with conflict copies instead of lost text.

### Phase 3: Compendium (in progress)
- **SRD 5.2** (2024 rules): every spell, monster, item, class, subclass, species, background, feat, feature, trait and condition, searchable offline.
- **Open5e books:** Kobold Press, Black Flag, Level Up A5E, SRD 5.1 and more, 20 books in all.
- The DM chooses the rules sources per campaign; players download only those.
- Same name in several sources: shown once by priority, with the other versions under "Also in".
- Credits screen with every licence notice.

## In progress

### Phase 3, continued
- **Server library:** your group's own entries (homebrew and content from books you own), with a form editor and JSON/YAML import and export. Library entries win name clashes.

## Planned

### Phase 3, remaining
- **Paste import:** paste a stat block or a whole chapter, review the parsed entries, accept some or all.
- **Bulk import:** large JSON files with mapping profiles, undoable as a batch; the setup-time import from an external repository.

### Phase 4: Character sheets
- Assisted sheets: the app computes modifiers, proficiency, saves, skills, spell slots and AC; you choose species, class, background and feats from the compendium.

### Phase 5: Dice
- Rolls from the sheet, shared roll log, secret DM rolls, physical dice entry.

### Phase 6: Combat tracker
- Initiative, turns, HP and conditions, with exact, vague or hidden monster health for players.

### Phase 7: Discord login
- Log in with Discord and link it to an existing account.

### Later
- Android app.
- A Discord bot that rolls with sheet modifiers, mirrored with the app.
- Other game systems (e.g. Imperium Maledictum, Call of Cthulhu); every source is already tagged with its system.
- Homebrew game systems defined in the app.
- Scripting for custom behaviour per campaign.
- A modding and add-on system for the whole app.

## Known limitations

- Links in notes and the compendium aren't clickable yet.
- A campaign's notes must fit in one 4 MB sync message (several hundred typical notes).
- Notes typed offline are queued as one save per pause in typing.
- Rules glossary entries (weapon properties, masteries, skills) aren't in the compendium yet.
- The OGL notices of Open5e books are generated from their metadata, not copied from the books.
- Updating the app can reset its local cache, dropping changes still queued offline at that moment.

## Sizes

Measured on Linux, release builds.

| Artifact | Size |
|---|---:|
| Interface (WASM, after `wasm-opt -Oz`) | 940 KB (384 KB gzipped) |
| SRD 5.2 download | 200 KB |
| All 20 Open5e books | 1.6 MB |
| Server RAM, idle / peak (40 MP image upload) | 6 MB / 157 MB |
