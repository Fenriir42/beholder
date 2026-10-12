# 0005: Password auth and rotating session tokens

## Context

SPEC §4: username + password (Argon2id) now, Discord later on the same account; opaque server-side tokens, rotated, stored in the OS keyring. Clients drop off the network often, and the server has 1 GB of RAM.

## Decision

- **Accounts:** `users` + `password_credentials`; `external_identities(provider, subject)` exists empty for Discord. Usernames are unique case-insensitively (`COLLATE NOCASE`).
- **Hashing:** Argon2id, m = 19 MiB, t = 2, p = 1 (OWASP baseline), on the blocking pool behind a 2-permit semaphore (≤ ~40 MiB). Unknown usernames are verified against a dummy hash so timing doesn't reveal which names exist.
- **Tokens:** 32 random bytes from the OS, sent as hex. Only their SHA-256 is stored. Sessions expire after 30 days without use (sliding).
- **Rotation:** at `Hello` (the only place a new token can be delivered), a token older than 24 h gets a replacement in `HelloOk.rotated_token`. The old token stays valid until the new one is used once; then the old row is deleted. If the client reconnects with the old token (it never received the new one), the unused replacement is discarded and a fresh one is issued. HTTP calls never rotate.
- **Rate limits (in memory):** login 20/min per IP and 10/min per username, registration 5/h per IP, joins 10/min per user, uploads 20/h per user. The client IP is the last `X-Forwarded-For` hop (appended by Caddy), falling back to the socket address.
- **Registration is open** (rate-limited). A user sees only campaigns they create or join with an invite code.

## Alternatives

- **JWTs:** can't be revoked without a server-side list, which is what we have anyway.
- **Rotating on every connect:** more database writes, and more chances to lose the new token on a flaky link.
- **A signup code:** one env var if the group wants it; not needed yet.

## Consequences

A stolen token is usable until it expires or the user logs out; rotation limits how long an old copy works once the real client reconnects.
