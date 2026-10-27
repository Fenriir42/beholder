use core::fmt;

use serde::{Deserialize, Serialize};

use crate::ids::UserId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum ValidationError {
    #[error("invalid length")]
    Length,
    #[error("invalid character")]
    Character,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Username(String);

impl Username {
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

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DisplayName(String);

impl DisplayName {
    pub fn new(s: &str) -> Result<Self, ValidationError> {
        clean_text(s, 40).map(Self)
    }

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

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Password(String);

impl Password {
    pub fn new(s: String) -> Result<Self, ValidationError> {
        if !(8..=256).contains(&s.len()) {
            return Err(ValidationError::Length);
        }
        Ok(Self(s))
    }

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub username: Username,
    pub display_name: DisplayName,
}

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
