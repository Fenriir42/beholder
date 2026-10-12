//! Strongly typed identifiers, sequence numbers and timestamps.

use core::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! uuid_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            /// The 16 raw bytes, used as the SQLite key.
            pub fn as_bytes(&self) -> &[u8; 16] {
                self.0.as_bytes()
            }

            /// Rebuilds the id from its 16 raw bytes.
            pub fn from_bytes(bytes: [u8; 16]) -> Self {
                Self(Uuid::from_bytes(bytes))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

uuid_id!(
    /// A user account.
    UserId
);
uuid_id!(
    /// A campaign.
    CampaignId
);
uuid_id!(
    /// A client command; a UUIDv7 so the server can tell its age when deduplicating.
    CommandId
);

impl CommandId {
    /// Unix milliseconds embedded in a UUIDv7, or `None` for any other version.
    pub fn unix_millis(&self) -> Option<u64> {
        if self.0.get_version_num() != 7 {
            return None;
        }
        let b = self.0.as_bytes();
        Some(u64::from_be_bytes([
            0, 0, b[0], b[1], b[2], b[3], b[4], b[5],
        ]))
    }
}

/// Position of an event in a campaign's log. Starts at 1; 0 means "nothing seen yet".
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Seq(pub u64);

/// Milliseconds since the Unix epoch, UTC.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct UnixMillis(pub i64);

/// SHA-256 of an uploaded image's original bytes; images are content-addressed by it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ImageHash(pub [u8; 32]);

impl ImageHash {
    /// Lowercase hex, as used in image URLs and file names.
    pub fn to_hex(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut s = String::with_capacity(64);
        for b in self.0 {
            s.push(HEX[usize::from(b >> 4)] as char);
            s.push(HEX[usize::from(b & 0xf)] as char);
        }
        s
    }

    /// Parses 64 hex digits (either case).
    pub fn from_hex(s: &str) -> Option<Self> {
        let bytes = s.as_bytes();
        if bytes.len() != 64 {
            return None;
        }
        let mut out = [0u8; 32];
        for (o, pair) in out.iter_mut().zip(bytes.as_chunks::<2>().0) {
            *o = (hex_val(pair[0])? << 4) | hex_val(pair[1])?;
        }
        Some(Self(out))
    }
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

impl fmt::Debug for ImageHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ImageHash({})", self.to_hex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_hash_hex_round_trip() {
        let mut raw = [0u8; 32];
        for (i, b) in raw.iter_mut().enumerate() {
            *b = (i * 7 + 3) as u8;
        }
        let h = ImageHash(raw);
        let hex = h.to_hex();
        assert_eq!(hex.len(), 64);
        assert_eq!(ImageHash::from_hex(&hex), Some(h));
        assert_eq!(ImageHash::from_hex(&hex.to_uppercase()), Some(h));
        assert_eq!(ImageHash::from_hex("zz"), None);
        assert_eq!(ImageHash::from_hex(&"g".repeat(64)), None);
    }

    #[test]
    fn command_id_reads_v7_timestamp() {
        // 0x0190_0000_0000 ms, version nibble 7, variant bits 10.
        let id = CommandId(Uuid::from_bytes([
            0x01, 0x90, 0, 0, 0, 0, 0x70, 0, 0x80, 0, 0, 0, 0, 0, 0, 1,
        ]));
        assert_eq!(id.unix_millis(), Some(0x0190_0000_0000));
        assert_eq!(CommandId(Uuid::nil()).unix_millis(), None);
    }
}
