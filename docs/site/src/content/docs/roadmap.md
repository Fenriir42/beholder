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
- English and French interface, switchable at any time; the whole interface scales with Ctrl + and Ctrl −.
- Server image, Docker Compose with Caddy, installers built by CI.

### Phase 2: Notes and handouts
- Markdown notes with images.
- Audiences: private, DM only, whole party, or chosen members. The server never sends a note to someone outside its audience.
- One-click **Reveal** to the party; a revealed handout appears for everyone at once.
- Collaborative editing for shared notes, with conflict copies instead of lost text.

### Phase 3: Compendium
- **SRD 5.2** (2024 rules): every spell, monster, item, class, subclass, species, background, feat, feature, trait and condition, searchable offline.
- **Open5e books:** Kobold Press, Black Flag, Level Up A5E, SRD 5.1 and more, 20 books in all.
- The DM chooses the rules sources per campaign; players download only those.
- Same name in several sources: shown once by priority, with the other versions under "Also in".
- Credits screen with every licence notice.
- **Server library:** your group's own entries (homebrew and content from books you own), shared on your server, with an editor per entry type, JSON and YAML import and export, and priority over other sources. The DM can hide single entries from a campaign's players.
- **Paste import:** paste one entry or a whole chapter of spells, monsters, magic items, feats or subclasses, review the parsed entries with duplicates flagged, edit any of them, and accept some or all.
- **Large imports:** tens of thousands of entries per file, sent in batches, with the option to replace entries already in the library, undoing a whole import in one action (keeping entries edited since), and JSON files of any shape read through saved, exportable mapping profiles with a summary review before importing. A converter turns the `dnd-data` JSON files into library files, and the library file format is a documented standard with a JSON Schema.
- **Setup import:** set `BEHOLDER_IMPORT_REPO` and the server imports a dnd-data style repository once, as one undoable import.

## In progress

- **Character sheets (Phase 4):** characters with an assisted sheet (computed modifiers, saves, skills, AC, spell slots including multiclass and pact magic), compendium links, edits by the owner and the DM, and summaries for the rest of the party. Export and import as JSON files. Still to come: an optional guided level-up.
- **Dice (Phase 5):** rolls made on the server from the sheet (saves, skills, checks, initiative, attacks, damage and healing at any slot level, advantage, critical damage) or typed by hand, a shared log of the last 50 rolls, rolls to the DM and secret DM rolls, and physical dice entry.
- **Combat tracker and battle map (Phase 6):** encounters prepared ahead and run one at a time, combatants from the party, the compendium or typed in, initiative typed or rolled through the dice log, turns and rounds, damage and healing, conditions that count down, concentration, and exact, vague or hidden monster health for players. A square-grid map with walls, zones and tokens: the DM controls everything and can hide any shape, players drag their own token, and the DM can lock the map or show moves live while dragging. Terrain (difficult, water, cover, obscured), hazards with readable, rollable notes, doors and objects, and area templates (circle, cone, line, cube) that players can also place for their spells. An acting panel lets the DM play any monster from its stat block or any character from its sheet, and the DM can preview the combat page as any player. Spells and attacks are aimed at targets and roll only the caster's side, the DM applies damage or healing (full, half or double) with one click, characters' HP stays in sync with their sheets, and each combatant's action, bonus action, reaction and movement are tracked, with movement counted from token drags.

## Planned

### Phase 4: Character sheets
- Assisted sheets: the app computes modifiers, proficiency, saves, skills, spell slots and AC; you choose species, class, background and feats from the compendium.


### Phase 6: Combat tracker and battle map
- Later for the map: background images, fog of war, measuring, token images.

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
- Library export saves to the Downloads folder; there's no save dialog yet.
- Converted `dnd-data` descriptions get their tables, headings and lists back where the layout is recognised; the rest stays flattened text. Monster stat blocks in those files have no AC, hit points or ability scores.
- The `dnd-data` files hold almost no subclass rules: only the 12 Free Basic Rules subclasses become subclass entries.
- Hiding single entries works for the server library only, not for the SRD or Open5e books.
- After the protocol 10 update, rolls stored in the older format drop out of the dice log once.

## Sizes

Measured on Linux, release builds.

| Artifact | Size |
|---|---:|
| Interface (WASM, after `wasm-opt -Oz`) | 1.05 MB (426 KB gzipped) |
| Desktop app binary (Linux) | 6.1 MB |
| SRD 5.2 download | 200 KB |
| All 20 Open5e books | 1.6 MB |
| Server RAM, idle / peak (40 MP image upload) | 6 MB / 157 MB |
