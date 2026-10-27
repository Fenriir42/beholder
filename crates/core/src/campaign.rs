use core::fmt;

use serde::{Deserialize, Serialize};

use crate::compendium::{LIBRARY, SRD};
use crate::ids::{CampaignId, ImageHash, LibraryId, NoteId, UserId};
use crate::note::{Audience, Note, NoteBody, NoteTitle};
use crate::user::{DisplayName, ValidationError, clean_text};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Dm,
    Player,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CombatVisibility {
    #[default]
    Exact,
    Vague,
    Hidden,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CampaignSettings {
    pub dm_can_read_private_notes: bool,
    pub combat_visibility: CombatVisibility,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CampaignTitle(String);

impl CampaignTitle {
    pub fn new(s: &str) -> Result<Self, ValidationError> {
        clean_text(s, 80).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CampaignTitle {
    type Error = ValidationError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(&s)
    }
}

impl From<CampaignTitle> for String {
    fn from(t: CampaignTitle) -> Self {
        t.0
    }
}

const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "[u8; 8]", into = "[u8; 8]")]
pub struct InviteCode([u8; 8]);

impl InviteCode {
    pub fn from_random(bytes: [u8; 5]) -> Self {
        let mut n = bytes.iter().fold(0u64, |acc, &b| (acc << 8) | u64::from(b));
        let mut out = [0u8; 8];
        for c in out.iter_mut().rev() {
            *c = CROCKFORD[(n & 31) as usize];
            n >>= 5;
        }
        Self(out)
    }

    pub fn parse(input: &str) -> Result<Self, ValidationError> {
        let mut out = [0u8; 8];
        let mut len = 0;
        for c in input.bytes() {
            let c = match c.to_ascii_uppercase() {
                b' ' | b'-' => continue,
                b'I' | b'L' => b'1',
                b'O' => b'0',
                c => c,
            };
            if !CROCKFORD.contains(&c) {
                return Err(ValidationError::Character);
            }
            let slot = out.get_mut(len).ok_or(ValidationError::Length)?;
            *slot = c;
            len += 1;
        }
        if len != 8 {
            return Err(ValidationError::Length);
        }
        Ok(Self(out))
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.0).unwrap_or_default()
    }
}

impl TryFrom<[u8; 8]> for InviteCode {
    type Error = ValidationError;
    fn try_from(b: [u8; 8]) -> Result<Self, Self::Error> {
        if b.iter().all(|c| CROCKFORD.contains(c)) {
            Ok(Self(b))
        } else {
            Err(ValidationError::Character)
        }
    }
}

impl From<InviteCode> for [u8; 8] {
    fn from(c: InviteCode) -> Self {
        c.0
    }
}

impl fmt::Display for InviteCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.as_str();
        write!(f, "{}-{}", &s[..4], &s[4..])
    }
}

impl fmt::Debug for InviteCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "InviteCode({self})")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub user_id: UserId,
    pub display_name: DisplayName,
    pub role: Role,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CampaignOp {
    Rename { title: CampaignTitle },
    SetCover { cover: Option<ImageHash> },
    RegenerateInvite,
    UpdateSettings(CampaignSettings),
    SetSources { sources: Vec<String> },
    HideEntry { id: LibraryId, hidden: bool },
}

pub const MAX_SOURCES: usize = 64;

pub fn set_hidden(list: &mut Vec<LibraryId>, id: LibraryId, hidden: bool) {
    match list.binary_search(&id) {
        Ok(i) if !hidden => {
            list.remove(i);
        }
        Err(i) if hidden => list.insert(i, id),
        _ => {}
    }
}

impl CampaignOp {
    pub fn predicted_event(&self) -> Option<CampaignEvent> {
        match self {
            Self::Rename { title } => Some(CampaignEvent::Renamed {
                title: title.clone(),
            }),
            Self::SetCover { cover } => Some(CampaignEvent::CoverChanged { cover: *cover }),
            Self::RegenerateInvite => None,
            Self::UpdateSettings(s) => Some(CampaignEvent::SettingsChanged { settings: *s }),
            Self::SetSources { sources } => {
                let mut unique: Vec<String> = Vec::with_capacity(sources.len());
                for s in sources {
                    if !unique.contains(s) {
                        unique.push(s.clone());
                    }
                }
                (unique.len() <= MAX_SOURCES)
                    .then_some(CampaignEvent::SourcesChanged { sources: unique })
            }
            Self::HideEntry { id, hidden } => Some(CampaignEvent::EntryHidden {
                id: *id,
                hidden: *hidden,
            }),
        }
    }
}

// Stored as postcard: only append variants, never change one (ADR-0003).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CampaignEvent {
    Created {
        title: CampaignTitle,
        dm: Member,
    },
    Renamed {
        title: CampaignTitle,
    },
    CoverChanged {
        cover: Option<ImageHash>,
    },
    MemberJoined {
        member: Member,
    },
    InviteRegenerated {
        code: InviteCode,
    },
    SettingsChanged {
        settings: CampaignSettings,
    },
    NoteCreated {
        note: Note,
    },
    NoteEdited {
        id: NoteId,
        title: NoteTitle,
        body: NoteBody,
        version: u32,
    },
    NoteAudienceChanged {
        id: NoteId,
        audience: Audience,
        previous: Audience,
    },
    NoteDeleted {
        id: NoteId,
    },
    SourcesChanged {
        sources: Vec<String>,
    },
    EntryHidden {
        id: LibraryId,
        hidden: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Campaign {
    pub id: CampaignId,
    pub title: CampaignTitle,
    pub cover: Option<ImageHash>,
    pub settings: CampaignSettings,
    pub invite_code: Option<InviteCode>,
    pub members: Vec<Member>,
    pub notes: Vec<Note>,
    pub sources: Vec<String>,
    pub hidden: Vec<LibraryId>,
}

impl Campaign {
    pub fn new(id: CampaignId, title: CampaignTitle, dm: Member) -> Self {
        Self {
            id,
            title,
            cover: None,
            settings: CampaignSettings::default(),
            invite_code: None,
            members: vec![dm],
            notes: Vec::new(),
            sources: vec![SRD.to_owned(), LIBRARY.to_owned()],
            hidden: Vec::new(),
        }
    }

    pub fn apply(&mut self, event: &CampaignEvent) {
        match event {
            CampaignEvent::Created { title, dm } => {
                *self = Self::new(self.id, title.clone(), dm.clone());
            }
            CampaignEvent::Renamed { title } => self.title = title.clone(),
            CampaignEvent::CoverChanged { cover } => self.cover = *cover,
            CampaignEvent::MemberJoined { member } => {
                match self
                    .members
                    .iter_mut()
                    .find(|m| m.user_id == member.user_id)
                {
                    Some(m) => *m = member.clone(),
                    None => self.members.push(member.clone()),
                }
            }
            CampaignEvent::InviteRegenerated { code } => self.invite_code = Some(*code),
            CampaignEvent::SettingsChanged { settings } => self.settings = *settings,
            CampaignEvent::NoteCreated { note } => {
                match self.notes.binary_search_by_key(&note.id, |n| n.id) {
                    Ok(i) => self.notes[i] = note.clone(),
                    Err(i) => self.notes.insert(i, note.clone()),
                }
            }
            CampaignEvent::NoteEdited {
                id,
                title,
                body,
                version,
            } => {
                if let Some(n) = self.note_mut(*id) {
                    n.title = title.clone();
                    n.body = body.clone();
                    n.version = *version;
                }
            }
            CampaignEvent::NoteAudienceChanged { id, audience, .. } => {
                if let Some(n) = self.note_mut(*id) {
                    n.audience = audience.clone();
                }
            }
            CampaignEvent::NoteDeleted { id } => {
                if let Ok(i) = self.notes.binary_search_by_key(id, |n| n.id) {
                    self.notes.remove(i);
                }
            }
            CampaignEvent::SourcesChanged { sources } => self.sources = sources.clone(),
            CampaignEvent::EntryHidden { id, hidden } => set_hidden(&mut self.hidden, *id, *hidden),
        }
    }

    pub fn note(&self, id: NoteId) -> Option<&Note> {
        self.notes
            .binary_search_by_key(&id, |n| n.id)
            .ok()
            .map(|i| &self.notes[i])
    }

    fn note_mut(&mut self, id: NoteId) -> Option<&mut Note> {
        self.notes
            .binary_search_by_key(&id, |n| n.id)
            .ok()
            .map(|i| &mut self.notes[i])
    }

    pub fn role_of(&self, user: UserId) -> Option<Role> {
        self.members
            .iter()
            .find(|m| m.user_id == user)
            .map(|m| m.role)
    }

    pub fn dm(&self) -> Option<&Member> {
        self.members.iter().find(|m| m.role == Role::Dm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn member(n: u8, role: Role) -> Member {
        Member {
            user_id: UserId(Uuid::from_bytes([n; 16])),
            display_name: DisplayName::new(&format!("user{n}")).unwrap(),
            role,
        }
    }

    #[test]
    fn invite_code_generation_is_canonical() {
        let c = InviteCode::from_random([0, 0, 0, 0, 0]);
        assert_eq!(c.as_str(), "00000000");
        let c = InviteCode::from_random([0xff; 5]);
        assert_eq!(c.as_str(), "ZZZZZZZZ");
        let c = InviteCode::from_random([0x12, 0x34, 0x56, 0x78, 0x9a]);
        assert_eq!(InviteCode::parse(c.as_str()), Ok(c));
        assert_eq!(InviteCode::parse(&c.to_string()), Ok(c));
    }

    #[test]
    fn invite_code_parse_is_forgiving_but_strict_on_length() {
        let c = InviteCode::parse("AB1D-EF0H").unwrap();
        assert_eq!(InviteCode::parse("abld efoh"), Ok(c));
        assert_eq!(InviteCode::parse("aBiD-eFoH"), Ok(c));
        assert_eq!(c.to_string(), "AB1D-EF0H");
        assert_eq!(InviteCode::parse("ABCDEFG"), Err(ValidationError::Length));
        assert_eq!(InviteCode::parse("ABCDEFGHJ"), Err(ValidationError::Length));
        assert_eq!(
            InviteCode::parse("ABCDEFGU"),
            Err(ValidationError::Character)
        );
        assert_eq!(
            InviteCode::try_from(*b"abcdefgh"),
            Err(ValidationError::Character)
        );
    }

    #[test]
    fn title_validation() {
        assert_eq!(CampaignTitle::new("  Curse ").unwrap().as_str(), "Curse");
        assert_eq!(CampaignTitle::new(""), Err(ValidationError::Length));
        assert_eq!(
            CampaignTitle::new(&"x".repeat(81)),
            Err(ValidationError::Length)
        );
    }

    #[test]
    fn events_fold_into_state() {
        let id = CampaignId(Uuid::from_bytes([9; 16]));
        let dm = member(1, Role::Dm);
        let p = member(2, Role::Player);
        let mut c = Campaign::new(id, CampaignTitle::new("A").unwrap(), dm.clone());
        let code = InviteCode::from_random([1, 2, 3, 4, 5]);
        let settings = CampaignSettings {
            dm_can_read_private_notes: true,
            combat_visibility: CombatVisibility::Vague,
        };
        for e in [
            CampaignEvent::Renamed {
                title: CampaignTitle::new("B").unwrap(),
            },
            CampaignEvent::CoverChanged {
                cover: Some(ImageHash([7; 32])),
            },
            CampaignEvent::MemberJoined { member: p.clone() },
            CampaignEvent::MemberJoined { member: p.clone() },
            CampaignEvent::InviteRegenerated { code },
            CampaignEvent::SettingsChanged { settings },
        ] {
            c.apply(&e);
        }
        assert_eq!(c.title.as_str(), "B");
        assert_eq!(c.cover, Some(ImageHash([7; 32])));
        assert_eq!(c.members, vec![dm.clone(), p.clone()]);
        assert_eq!(c.invite_code, Some(code));
        assert_eq!(c.settings, settings);
        assert_eq!(c.role_of(p.user_id), Some(Role::Player));
        assert_eq!(c.dm(), Some(&dm));
        assert_eq!(c.role_of(UserId(Uuid::nil())), None);
    }

    #[test]
    fn note_events_fold_in_id_order() {
        let dm = member(1, Role::Dm);
        let mut c = Campaign::new(
            CampaignId(Uuid::from_bytes([9; 16])),
            CampaignTitle::new("A").unwrap(),
            dm.clone(),
        );
        let note = |n: u8| Note {
            id: NoteId(Uuid::from_bytes([n; 16])),
            author: dm.user_id,
            title: NoteTitle::new("T").unwrap(),
            body: NoteBody::default(),
            audience: Audience::Private,
            version: 1,
            conflict_of: None,
        };
        let (b, a) = (note(2), note(1));
        for e in [
            CampaignEvent::NoteCreated { note: b.clone() },
            CampaignEvent::NoteCreated { note: a.clone() },
            CampaignEvent::NoteEdited {
                id: a.id,
                title: NoteTitle::new("Edited").unwrap(),
                body: NoteBody::new("x".into()).unwrap(),
                version: 2,
            },
            CampaignEvent::NoteAudienceChanged {
                id: b.id,
                audience: Audience::Party,
                previous: Audience::Private,
            },
            CampaignEvent::NoteCreated { note: b.clone() },
        ] {
            c.apply(&e);
        }
        assert_eq!(c.notes.len(), 2);
        assert_eq!(c.notes[0].id, a.id, "sorted by id, not arrival");
        assert_eq!(c.notes[0].title.as_str(), "Edited");
        assert_eq!(c.notes[0].version, 2);
        assert_eq!(c.note(b.id), Some(&b), "a repeated create is an upsert");
        c.apply(&CampaignEvent::NoteDeleted { id: a.id });
        c.apply(&CampaignEvent::NoteDeleted { id: a.id });
        assert_eq!(c.notes, vec![b]);
    }

    #[test]
    fn predicted_events_match_ops() {
        let title = CampaignTitle::new("X").unwrap();
        assert_eq!(
            CampaignOp::Rename {
                title: title.clone()
            }
            .predicted_event(),
            Some(CampaignEvent::Renamed { title })
        );
        assert_eq!(CampaignOp::RegenerateInvite.predicted_event(), None);
        let s = |v: &[&str]| v.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            CampaignOp::SetSources {
                sources: s(&["tob3", "srd-5.2", "tob3"])
            }
            .predicted_event(),
            Some(CampaignEvent::SourcesChanged {
                sources: s(&["tob3", "srd-5.2"])
            }),
            "duplicates are dropped, order kept"
        );
        let too_many = (0..=MAX_SOURCES).map(|i| i.to_string()).collect();
        assert_eq!(
            CampaignOp::SetSources { sources: too_many }.predicted_event(),
            None
        );
    }
}
