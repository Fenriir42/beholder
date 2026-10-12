# 0003: Wire protocol and frame format

## Context

SPEC §3: binary WebSocket frames, postcard-serialized enums, zstd above ~1 KB, a protocol version checked at handshake. Clients sit behind slow, lossy VPN links, so frames must be small and decoding must be safe against hostile input.

## Decision

- **Frame:** `[flags: u8][body]`. Bit 0 set means the body is zstd (level 3). Other bits are reserved; an unknown bit is a decode error.
- A body is compressed only when it is over 1,024 bytes *and* compression makes it smaller.
- `MAX_FRAME_BYTES` = 4 MiB, applied to the raw frame and to the decompressed body (`zstd::bulk::decompress` with a capacity bound), so a zip bomb can't allocate past it.
- **Version:** `PROTOCOL_VERSION` (u16) travels in `Hello` and in the `x-beholder-protocol` HTTP header, not in every frame. A mismatch gets `HelloErr::UpdateRequired` / `AuthError::UpdateRequired`.
- **HTTP bodies are postcard too**, so no JSON library is needed on either side.
- **Validation on decode:** domain newtypes (`CampaignTitle`, `Username`, `InviteCode`, …) use `serde(try_from)`, so an invalid value can't be deserialized into a valid type.
- **Evolution rule:** postcard is not self-describing. The only compatible change is **appending** an enum variant (never reorder, never add a struct field). Anything else bumps `PROTOCOL_VERSION`. The same rule applies to `CampaignEvent`, which the server stores as postcard: a changed event shape becomes a new variant (`FooV2`), and old variants stay decodable forever.
- The codec is behind the `codec` feature, so the message types build for wasm32 without the C zstd library. Server and client-core enable it.
- Every message variant has a round-trip test; an exhaustive `match` in the tests fails to compile when a variant is added without one.

## Alternatives

- **Version byte in every frame:** costs a byte per message for no benefit, since a connection can't change version mid-stream.
- **ruzstd (pure Rust):** its encoder compresses noticeably worse and the UI doesn't need the codec anyway.
- **JSON for HTTP:** a second serialization format and `serde_json` in both binaries.
- **Self-describing format (CBOR, MessagePack):** tolerates added fields but makes every frame larger, and frame size matters more here.

## Consequences

Wire changes need discipline: append-only variants, or a version bump with the "please update" path. Ping frames are 4 bytes of body; an idle connection costs a few hundred bytes per minute including WebSocket and TLS overhead.
