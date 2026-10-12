//! User accounts and credential validation.

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::ids::UserId;

/// Why a user-supplied value was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum ValidationError {
    /// The value is empty, too short or too long.
    #[error("invalid length")]
    Length,
    /// The value contains a character that isn't allowed.
    #[error("invalid character")]
    Character,
}

/// Login name: 3–32 ASCII letters, digits, `_`, `-` or `.`. Unique case-insensitively.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Username(String);

impl Username {
    /// Validates a login name.
    pub fn new(s: String) -> Result<Self, ValidationError> {
        if !(3..=32).contains(&s.len()) {
            return Err(ValidationError::Length);
        }
        if !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
        {
            return Err(ValidationError::Character);
        }
        Ok(Self(s))
    }

    /// The name as typed at registration.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Username {
    type Error = ValidationError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl From<Username> for String {
    fn from(u: Username) -> Self {
        u.0
    }
}

/// Name shown to other players: 1–40 characters after trimming, no control characters.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DisplayName(String);

impl DisplayName {
    /// Trims and validates a display name.
    pub fn new(s: &str) -> Result<Self, ValidationError> {
        clean_text(s, 40).map(Self)
    }

    /// The display name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for DisplayName {
    type Error = ValidationError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(&s)
    }
}

impl From<DisplayName> for String {
    fn from(d: DisplayName) -> Self {
        d.0
    }
}

/// A plaintext password in transit; `Debug` never prints it.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Password(String);

impl Password {
    /// Accepts 8–256 bytes; anything longer only slows hashing down.
    pub fn new(s: String) -> Result<Self, ValidationError> {
        if !(8..=256).contains(&s.len()) {
            return Err(ValidationError::Length);
        }
        Ok(Self(s))
    }

    /// The plaintext, for hashing.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Password {
    type Error = ValidationError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl From<Password> for String {
    fn from(p: Password) -> Self {
        p.0
    }
}

impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Password(***)")
    }
}

/// The public view of an account.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    /// Account id.
    pub id: UserId,
    /// Login name.
    pub username: Username,
    /// Name shown to others.
    pub display_name: DisplayName,
}

/// Trims `s` and checks it is 1..=`max` chars with no control characters.
pub(crate) fn clean_text(s: &str, max: usize) -> Result<String, ValidationError> {
    let t = s.trim();
    let n = t.chars().count();
    if n == 0 || n > max {
        return Err(ValidationError::Length);
    }
    if t.chars().any(char::is_control) {
        return Err(ValidationError::Character);
    }
    Ok(t.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn username_rules() {
        assert!(Username::new("bob".into()).is_ok());
        assert!(Username::new("Jean-Luc.42_x".into()).is_ok());
        assert_eq!(Username::new("ab".into()), Err(ValidationError::Length));
        assert_eq!(Username::new("a".repeat(33)), Err(ValidationError::Length));
        assert_eq!(
            Username::new("bob smith".into()),
            Err(ValidationError::Character)
        );
        assert_eq!(
            Username::new("élise".into()),
            Err(ValidationError::Character)
        );
    }

    #[test]
    fn display_name_trims_and_counts_chars() {
        assert_eq!(DisplayName::new("  Élise  ").unwrap().as_str(), "Élise");
        assert!(DisplayName::new(&"é".repeat(40)).is_ok());
        assert_eq!(
            DisplayName::new(&"é".repeat(41)),
            Err(ValidationError::Length)
        );
        assert_eq!(DisplayName::new("   "), Err(ValidationError::Length));
        assert_eq!(DisplayName::new("a\nb"), Err(ValidationError::Character));
    }

    #[test]
    fn password_length_and_debug() {
        assert_eq!(Password::new("short".into()), Err(ValidationError::Length));
        let p = Password::new("correct horse".into()).unwrap();
        assert_eq!(format!("{p:?}"), "Password(***)");
    }
}
