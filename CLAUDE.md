# CLAUDE.md

This is a companion app for a D&D 5e campaign on the 2024 rules (SRD 5.2). It has a Rust server and a Tauri 2 + Leptos desktop client for Linux and Windows, with Android planned for later. Its users are a small group of friends who often connect over unstable networks (a VPN from China). The developer works on a weak laptop.

**The full spec is in `docs/SPEC.md`.** Before planning, read the section that covers your task; you don't need to re-read the whole file each time. If this file and the spec disagree, stop and ask. Past decisions are recorded as ADRs in `docs/decisions/`.

## Priorities

1. **Optimisation:** CPU, RAM, disk, network, binary and WASM size.
2. **Maintainability:** small crates, clear boundaries, tests.
3. **Rust wherever reasonable.**

When two of these conflict, name the tradeoff and propose an option instead of choosing silently.

## Repo map

| Path | Crate | Role |
|---|---|---|
| `crates/core` | `beholder-core` | Domain types, rules calculations, dice parser. No IO, no async, builds for wasm32 |
| `crates/protocol` | `beholder-protocol` | Wire messages, `PROTOCOL_VERSION`, postcard + zstd codec |
| `crates/server` | `beholder-server` | axum + SQLite server, permissions, event log, sync |
| `crates/client-core` | `beholder-client-core` | Connection manager, outbox, local cache. No Tauri dependency |
| `crates/e2e` | `beholder-e2e` | Test-only: end-to-end tests (flaky-network proxy) spanning server and client-core |
| `apps/desktop/ipc` | `beholder-ipc` | Serde DTOs shared by the UI (wasm32) and the Tauri shell |
| `apps/desktop/src-tauri` | `beholder-desktop` | Tauri shell: commands and events between the UI and client-core |
| `apps/desktop/ui` | `beholder-ui` | Leptos CSR app (wasm32) |
| `data/srd-5.2` | n/a | Vendored SRD JSON (pinned commit) and the attribution text |
| `deploy/` | n/a | Dockerfile, Caddyfile, docker-compose |

Update this map whenever a crate is added, renamed or moved.

## Commands

The laptop is slow, so always run the narrowest command that verifies the change.

- Format: `cargo fmt --all`
- Lint one crate: `cargo clippy -p <crate> --all-targets -- -D warnings`
- Test one crate: `cargo test -p <crate>` (filter by test name when you can)
- WASM compatibility, required after touching core, protocol or ui: `cargo check -p beholder-core -p beholder-protocol -p beholder-ui --target wasm32-unknown-unknown`
- Full check, only before a commit that touches several crates: `cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`

Add the real run, bundle and Docker commands here once Phase 1 creates them.

Don't run any of the following unless asked: release builds, `cargo tauri build` or other bundling, `cargo clean`, `cargo update`, or long-running processes (`cargo tauri dev`, `trunk serve`, the server). The user runs the app himself.

## Hard rules

- **The server filters what each client receives.** A client never gets data its user may not see: DM-only notes, hidden combatants, exact monster HP under `vague`, secret rolls, or other people's private notes. Hiding something in the UI is never enough. Every permission rule lives in the server's permissions module and has a case in the permission-matrix test.
- **The UI never touches the network.** The path is Leptos UI → Tauri commands/events → `client-core`, and only `client-core` talks to the server.
- **Every change goes through a command.** Each user action is a command with a UUIDv7 id that is safe to resend, because the server deduplicates. No code path may apply a change outside the command/event flow.
- **Protocol changes:** make them additive when you can. A breaking change bumps `PROTOCOL_VERSION`. Every message has a round-trip test.
- **No CDNs or Google services at runtime.** Fonts, icons and assets are bundled, since those services are blocked or slow in China.
- **Only SRD 5.2 content ships with the app**, with its CC-BY attribution. Non-SRD content is entered by users and never bundled.
- **No secrets in the repo.** Configuration comes from environment variables or git-ignored files.

## Rust conventions

- Errors: `thiserror` in libraries, `anyhow` only in binaries.
- No `unwrap()` or `expect()` outside tests and startup code. Make impossible states unrepresentable in the types instead of adding error paths for them.
- `beholder-core` is pure and deterministic: no IO, no async, and the RNG is passed in.
- Prefer borrowing to cloning, and plain structs and enums to trait hierarchies. Don't add a trait or generic that has only one implementation.
- No `unsafe`.
- Comments explain *why*, not what. Public items get a one-line doc comment.

## Dependencies

- Justify every new crate: why it's needed, what it costs in size and compile time, and which lighter alternative you rejected. Turn off default features and enable only what's needed.
- Look up the current stable version instead of guessing from memory.
- When a dependency is added to `beholder-ui` or `beholder-desktop`, report the WASM or binary size before and after.

## UI

- Take colours, sizes, radii and fonts from the design tokens (SPEC §6). Never hardcode them in components.
- Every user-facing string goes through Fluent. Add it to both `en` and `fr` in the same change.
- Touch targets are at least 44 px. Never convey status by colour alone.

## Workflow

- For anything bigger than a small fix, plan first: list the steps and how each will be verified. When the user asked for a plan, wait for approval.
- Make small commits that each compile and pass tests, using Conventional Commits (`feat(server): …`, `fix(client-core): …`). Never push, force-push or rewrite history unless asked.
- Write a short ADR in `docs/decisions/NNNN-title.md` for each significant decision: a new crate, a protocol, sync or storage design, or any deviation from SPEC. Cover the context, the decision, the alternatives and the consequences.
- When a task ends, report what changed, how you verified it, what you deferred, and any size changes.

---

## Behavioral guidelines

These guidelines favour caution over speed. For trivial tasks, use judgment.

### 1. Think before coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

- State your assumptions explicitly. If you're unsure, ask.
- If a request can be read more than one way, lay out the readings instead of picking one silently.
- If a simpler approach exists, say so, and push back when it's warranted.
- If something is unclear, stop, name what's confusing, and ask.

### 2. Simplicity first

**Write the minimum code that solves the problem, with nothing speculative.**

- Don't add features beyond what was asked, or features from later phases of the spec.
- Don't create abstractions for code used in one place.
- Don't add flexibility or configuration nobody requested.
- Don't add error handling for impossible scenarios; encode them in the types instead.
- If you wrote 200 lines and it could be 50, rewrite it.

Ask yourself whether a senior engineer would call it overcomplicated. If so, simplify.

### 3. Surgical changes

**Touch only what you must, and clean up only your own mess.**

- Don't "improve" nearby code, comments or formatting.
- Don't refactor code that isn't broken.
- Match the existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it rather than deleting it.
- Remove imports, variables and functions that *your* change made unused. Leave pre-existing dead code unless asked.

The test: every changed line traces directly back to the request.

### 4. Goal-driven execution

**Define success criteria, then loop until they're verified.**

Turn each task into a goal you can verify:
- "Add validation" becomes "write tests for invalid input, then make them pass".
- "Fix the bug" becomes "write a test that reproduces it, then make it pass".
- "Refactor X" becomes "tests pass before and after, with no behaviour change".
- "Players must not see DM notes" becomes "add permission-matrix cases, then make them pass".

For multi-step tasks, state a short plan:
```
1. [step] → verify: cargo test -p beholder-server permissions
2. [step] → verify: cargo check -p beholder-ui --target wasm32-unknown-unknown
3. [step] → verify: [check]
```

Strong criteria let you loop on your own. Weak ones ("make it work") need constant clarification.

---

**These guidelines are working if** diffs have fewer unnecessary changes, fewer rewrites come from overcomplication, and clarifying questions come before implementation rather than after mistakes.
