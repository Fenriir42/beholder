use serde::{Deserialize, Serialize};

use crate::campaign::CampaignEvent;
use crate::ids::{NoteId, UserId};
use crate::user::{ValidationError, clean_text};

pub const MAX_BODY_BYTES: usize = 64 * 1024;

pub const MAX_AUDIENCE: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Audience {
    Private,
    Party,
    Users(Vec<UserId>),
}

impl Audience {
    pub fn normalized(self, author: UserId) -> Result<Self, ValidationError> {
        let Self::Users(mut users) = self else {
            return Ok(self);
        };
        users.retain(|u| *u != author);
        users.sort_unstable();
        users.dedup();
        match users.len() {
            0 => Ok(Self::Private),
            n if n > MAX_AUDIENCE => Err(ValidationError::Length),
            _ => Ok(Self::Users(users)),
        }
    }

    pub fn includes(&self, user: UserId) -> bool {
        match self {
            Self::Private => false,
            Self::Party => true,
            Self::Users(users) => users.contains(&user),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NoteTitle(String);

impl NoteTitle {
    pub fn new(s: &str) -> Result<Self, ValidationError> {
        clean_text(s, 120).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for NoteTitle {
    type Error = ValidationError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(&s)
    }
}

impl From<NoteTitle> for String {
    fn from(t: NoteTitle) -> Self {
        t.0
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NoteBody(String);

impl NoteBody {
    pub fn new(mut s: String) -> Result<Self, ValidationError> {
        if s.contains('\r') {
            s.retain(|c| c != '\r');
        }
        if s.len() > MAX_BODY_BYTES {
            return Err(ValidationError::Length);
        }
        if s.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
            return Err(ValidationError::Character);
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for NoteBody {
    type Error = ValidationError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl From<NoteBody> for String {
    fn from(b: NoteBody) -> Self {
        b.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: NoteId,
    pub author: UserId,
    pub title: NoteTitle,
    pub body: NoteBody,
    pub audience: Audience,
    pub version: u32,
    pub conflict_of: Option<NoteId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoteOp {
    Create {
        id: NoteId,
        title: NoteTitle,
        body: NoteBody,
        audience: Audience,
    },
    Edit {
        id: NoteId,
        base_version: u32,
        title: NoteTitle,
        body: NoteBody,
    },
    SetAudience {
        id: NoteId,
        audience: Audience,
    },
    Delete {
        id: NoteId,
    },
}

impl NoteOp {
    pub fn note_id(&self) -> NoteId {
        match self {
            Self::Create { id, .. }
            | Self::Edit { id, .. }
            | Self::SetAudience { id, .. }
            | Self::Delete { id } => *id,
        }
    }

    pub fn predicted_event(&self, actor: UserId, current: Option<&Note>) -> Option<CampaignEvent> {
        match (self, current) {
            (
                Self::Create {
                    id,
                    title,
                    body,
                    audience,
                },
                None,
            ) => Some(CampaignEvent::NoteCreated {
                note: Note {
                    id: *id,
                    author: actor,
                    title: title.clone(),
                    body: body.clone(),
                    audience: audience.clone().normalized(actor).ok()?,
                    version: 1,
                    conflict_of: None,
                },
            }),
            (
                Self::Edit {
                    id,
                    base_version,
                    title,
                    body,
                },
                Some(note),
            ) if note.version == *base_version => Some(CampaignEvent::NoteEdited {
                id: *id,
                title: title.clone(),
                body: body.clone(),
                version: base_version.checked_add(1)?,
            }),
            (Self::SetAudience { id, audience }, Some(note)) => {
                let audience = audience.clone().normalized(note.author).ok()?;
                (audience != note.audience).then(|| CampaignEvent::NoteAudienceChanged {
                    id: *id,
                    audience,
                    previous: note.audience.clone(),
                })
            }
            (Self::Delete { id }, Some(_)) => Some(CampaignEvent::NoteDeleted { id: *id }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn user(n: u8) -> UserId {
        UserId(Uuid::from_bytes([n; 16]))
    }

    fn note(author: UserId, audience: Audience) -> Note {
        Note {
            id: NoteId(Uuid::from_bytes([5; 16])),
            author,
            title: NoteTitle::new("Map").unwrap(),
            body: NoteBody::default(),
            audience,
            version: 3,
            conflict_of: None,
        }
    }

    #[test]
    fn audience_normalization() {
        let a = user(1);
        assert_eq!(
            Audience::Users(vec![user(3), a, user(2), user(3)]).normalized(a),
            Ok(Audience::Users(vec![user(2), user(3)]))
        );
        assert_eq!(
            Audience::Users(vec![a]).normalized(a),
            Ok(Audience::Private)
        );
        assert_eq!(Audience::Party.normalized(a), Ok(Audience::Party));
        let many = (0..=MAX_AUDIENCE as u8 + 1).map(user).collect();
        assert_eq!(
            Audience::Users(many).normalized(a),
            Err(ValidationError::Length)
        );
    }

    #[test]
    fn body_rules() {
        assert_eq!(
            NoteBody::new("a\r\nb\tc".into()).unwrap().as_str(),
            "a\nb\tc"
        );
        assert!(NoteBody::new(String::new()).is_ok());
        assert_eq!(
            NoteBody::new("x\u{7}".into()),
            Err(ValidationError::Character)
        );
        assert_eq!(
            NoteBody::new("x".repeat(MAX_BODY_BYTES + 1)),
            Err(ValidationError::Length)
        );
        assert!(NoteBody::new("x".repeat(MAX_BODY_BYTES)).is_ok());
        assert_eq!(NoteTitle::new(" "), Err(ValidationError::Length));
    }

    #[test]
    fn predictions() {
        let author = user(1);
        let n = note(author, Audience::Private);
        let edit = |base| NoteOp::Edit {
            id: n.id,
            base_version: base,
            title: n.title.clone(),
            body: NoteBody::new("new".into()).unwrap(),
        };
        assert!(matches!(
            edit(3).predicted_event(user(2), Some(&n)),
            Some(CampaignEvent::NoteEdited { version: 4, .. })
        ));
        assert_eq!(edit(2).predicted_event(author, Some(&n)), None);
        assert_eq!(edit(3).predicted_event(author, None), None);

        let reveal = NoteOp::SetAudience {
            id: n.id,
            audience: Audience::Party,
        };
        assert_eq!(
            reveal.predicted_event(author, Some(&n)),
            Some(CampaignEvent::NoteAudienceChanged {
                id: n.id,
                audience: Audience::Party,
                previous: Audience::Private,
            })
        );
        let same = NoteOp::SetAudience {
            id: n.id,
            audience: Audience::Users(vec![author]),
        };
        assert_eq!(same.predicted_event(author, Some(&n)), None);

        let create = NoteOp::Create {
            id: n.id,
            title: n.title.clone(),
            body: NoteBody::default(),
            audience: Audience::Private,
        };
        assert_eq!(
            create.predicted_event(author, None),
            Some(CampaignEvent::NoteCreated {
                note: Note {
                    version: 1,
                    ..n.clone()
                }
            })
        );
        assert_eq!(create.predicted_event(author, Some(&n)), None);
        assert_eq!(
            NoteOp::Delete { id: n.id }.predicted_event(author, Some(&n)),
            Some(CampaignEvent::NoteDeleted { id: n.id })
        );
    }
}
