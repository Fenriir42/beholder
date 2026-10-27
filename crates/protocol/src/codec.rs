use serde::Serialize;
use serde::de::DeserializeOwned;

pub const COMPRESS_THRESHOLD: usize = 1024;

pub const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;

const FLAG_ZSTD: u8 = 0b0000_0001;

const ZSTD_LEVEL: i32 = 3;

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("empty frame")]
    Empty,
    #[error("unknown frame flags {0:#04x}")]
    UnknownFlags(u8),
    #[error("frame too large")]
    TooLarge,
    #[error("zstd: {0}")]
    Zstd(#[from] std::io::Error),
    #[error("postcard: {0}")]
    Postcard(#[from] postcard::Error),
}

pub fn encode<T: Serialize>(msg: &T) -> Result<Vec<u8>, CodecError> {
    let frame = postcard::to_extend(msg, vec![0u8])?;
    let body = &frame[1..];
    if body.len() > MAX_FRAME_BYTES {
        return Err(CodecError::TooLarge);
    }
    if body.len() > COMPRESS_THRESHOLD {
        let z = zstd::bulk::compress(body, ZSTD_LEVEL)?;
        if z.len() < body.len() {
            let mut out = Vec::with_capacity(z.len() + 1);
            out.push(FLAG_ZSTD);
            out.extend_from_slice(&z);
            return Ok(out);
        }
    }
    Ok(frame)
}

pub fn decode<T: DeserializeOwned>(frame: &[u8]) -> Result<T, CodecError> {
    if frame.len() > MAX_FRAME_BYTES {
        return Err(CodecError::TooLarge);
    }
    let (&flags, body) = frame.split_first().ok_or(CodecError::Empty)?;
    match flags {
        0 => Ok(postcard::from_bytes(body)?),
        FLAG_ZSTD => {
            // `decompress` refuses to grow past the capacity, so a zip bomb fails here.
            let raw =
                zstd::bulk::decompress(body, MAX_FRAME_BYTES).map_err(|_| CodecError::TooLarge)?;
            Ok(postcard::from_bytes(&raw)?)
        }
        other => Err(CodecError::UnknownFlags(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::*;
    use crate::*;
    use beholder_core::*;
    use uuid::Uuid;

    fn uid(n: u8) -> UserId {
        UserId(Uuid::from_bytes([n; 16]))
    }

    fn cid(n: u8) -> CampaignId {
        CampaignId(Uuid::from_bytes([n; 16]))
    }

    fn cmd_id(n: u8) -> CommandId {
        CommandId(Uuid::from_bytes([n; 16]))
    }

    fn member(n: u8, role: Role) -> Member {
        Member {
            user_id: uid(n),
            display_name: DisplayName::new(&format!("Member {n}")).unwrap(),
            role,
        }
    }

    fn user() -> User {
        User {
            id: uid(1),
            username: Username::new("elise".into()).unwrap(),
            display_name: DisplayName::new("Élise").unwrap(),
        }
    }

    fn campaign(members: u8) -> Campaign {
        let mut c = Campaign::new(
            cid(2),
            CampaignTitle::new("Curse of Strahd").unwrap(),
            member(1, Role::Dm),
        );
        for n in 2..members.saturating_add(2) {
            c.apply(&CampaignEvent::MemberJoined {
                member: member(n, Role::Player),
            });
        }
        c.cover = Some(ImageHash([3; 32]));
        c.invite_code = Some(InviteCode::from_random([1, 2, 3, 4, 5]));
        c.apply(&CampaignEvent::NoteCreated { note: note() });
        c
    }

    fn nid(n: u8) -> NoteId {
        NoteId(Uuid::from_bytes([n; 16]))
    }

    fn note() -> Note {
        Note {
            id: nid(4),
            author: uid(1),
            title: NoteTitle::new("Handout: the letter").unwrap(),
            body: NoteBody::new("# Dear *Ireena*\n![seal](img:00)".into()).unwrap(),
            audience: Audience::Users(vec![uid(2), uid(3)]),
            version: 7,
            conflict_of: Some(nid(5)),
        }
    }

    fn all_events() -> Vec<CampaignEvent> {
        vec![
            CampaignEvent::Created {
                title: CampaignTitle::new("A").unwrap(),
                dm: member(1, Role::Dm),
            },
            CampaignEvent::Renamed {
                title: CampaignTitle::new("B").unwrap(),
            },
            CampaignEvent::CoverChanged { cover: None },
            CampaignEvent::MemberJoined {
                member: member(2, Role::Player),
            },
            CampaignEvent::InviteRegenerated {
                code: InviteCode::from_random([9; 5]),
            },
            CampaignEvent::SettingsChanged {
                settings: CampaignSettings {
                    dm_can_read_private_notes: true,
                    combat_visibility: CombatVisibility::Hidden,
                },
            },
            CampaignEvent::NoteCreated { note: note() },
            CampaignEvent::NoteEdited {
                id: nid(4),
                title: NoteTitle::new("Letter").unwrap(),
                body: NoteBody::new("Burn after reading.".into()).unwrap(),
                version: 8,
            },
            CampaignEvent::NoteAudienceChanged {
                id: nid(4),
                audience: Audience::Party,
                previous: Audience::Private,
            },
            CampaignEvent::NoteDeleted { id: nid(4) },
            CampaignEvent::SourcesChanged {
                sources: vec!["srd-5.2".into(), "tob3".into()],
            },
        ]
    }

    fn client_msgs() -> Vec<ClientMsg> {
        let ops = vec![
            CampaignOp::Rename {
                title: CampaignTitle::new("New").unwrap(),
            },
            CampaignOp::SetCover {
                cover: Some(ImageHash([4; 32])),
            },
            CampaignOp::RegenerateInvite,
            CampaignOp::UpdateSettings(CampaignSettings::default()),
            CampaignOp::SetSources {
                sources: vec!["srd-5.2".into(), "tob3".into()],
            },
        ];
        let mut msgs = vec![
            ClientMsg::Hello(Hello {
                protocol_version: PROTOCOL_VERSION,
                session_token: SessionToken([7; 32]),
                known: vec![(cid(2), Seq(41)), (cid(3), Seq(0))],
            }),
            ClientMsg::Ping { t: u64::MAX },
            ClientMsg::Command(Command {
                id: cmd_id(5),
                kind: CommandKind::CreateCampaign {
                    campaign_id: cid(6),
                    title: CampaignTitle::new("Tomb").unwrap(),
                },
            }),
            ClientMsg::Command(Command {
                id: cmd_id(6),
                kind: CommandKind::JoinCampaign {
                    code: InviteCode::parse("ABCD-EFGH").unwrap(),
                },
            }),
        ];
        msgs.extend(ops.into_iter().map(|op| {
            ClientMsg::Command(Command {
                id: cmd_id(7),
                kind: CommandKind::Campaign {
                    campaign_id: cid(2),
                    op,
                },
            })
        }));
        let note_ops = vec![
            NoteOp::Create {
                id: nid(4),
                title: NoteTitle::new("Map").unwrap(),
                body: NoteBody::default(),
                audience: Audience::Private,
            },
            NoteOp::Edit {
                id: nid(4),
                base_version: 3,
                title: NoteTitle::new("Map").unwrap(),
                body: NoteBody::new("North is *up*.".into()).unwrap(),
            },
            NoteOp::SetAudience {
                id: nid(4),
                audience: Audience::Users(vec![uid(2)]),
            },
            NoteOp::Delete { id: nid(4) },
        ];
        msgs.extend(note_ops.into_iter().map(|op| {
            ClientMsg::Command(Command {
                id: cmd_id(8),
                kind: CommandKind::Note {
                    campaign_id: cid(2),
                    op,
                },
            })
        }));
        msgs
    }

    fn server_msgs() -> Vec<ServerMsg> {
        let reasons = [
            RejectReason::Forbidden,
            RejectReason::NotFound,
            RejectReason::InvalidInviteCode,
            RejectReason::Invalid(ValidationError::Length),
            RejectReason::UnknownImage,
            RejectReason::IdConflict,
            RejectReason::Expired,
            RejectReason::RateLimited,
        ];
        let mut msgs = vec![
            ServerMsg::HelloOk(HelloOk {
                user: user(),
                campaigns: vec![cid(2)],
                rotated_token: Some(SessionToken([8; 32])),
            }),
            ServerMsg::HelloErr(HelloError::UpdateRequired { server_version: 2 }),
            ServerMsg::HelloErr(HelloError::InvalidSession),
            ServerMsg::Ack {
                id: cmd_id(1),
                seq: Some(Seq(10)),
            },
            ServerMsg::Ack {
                id: cmd_id(1),
                seq: None,
            },
            ServerMsg::Events {
                campaign_id: cid(2),
                events: all_events()
                    .into_iter()
                    .enumerate()
                    .map(|(i, event)| EventEnvelope {
                        seq: Seq(i as u64 + 1),
                        ts: UnixMillis(1_790_000_000_000),
                        actor: uid(1),
                        event,
                    })
                    .collect(),
            },
            ServerMsg::Snapshot {
                seq: Seq(12),
                campaign: campaign(3),
            },
            ServerMsg::CampaignRemoved {
                campaign_id: cid(2),
            },
            ServerMsg::Presence {
                campaign_id: cid(2),
                online: vec![uid(1), uid(2)],
            },
            ServerMsg::PresenceChanged {
                campaign_id: cid(2),
                user_id: uid(2),
                online: false,
            },
            ServerMsg::Pong { t: 0 },
        ];
        msgs.extend(reasons.map(|reason| ServerMsg::Reject {
            id: cmd_id(2),
            reason,
        }));
        msgs
    }

    #[allow(dead_code)]
    fn exhaustive(c: &ClientMsg, s: &ServerMsg, k: &CommandKind, e: &CampaignEvent, n: &NoteOp) {
        match c {
            ClientMsg::Hello(_) | ClientMsg::Command(_) | ClientMsg::Ping { .. } => {}
        }
        match k {
            CommandKind::CreateCampaign { .. }
            | CommandKind::JoinCampaign { .. }
            | CommandKind::Campaign { .. }
            | CommandKind::Note { .. } => {}
        }
        match e {
            CampaignEvent::Created { .. }
            | CampaignEvent::Renamed { .. }
            | CampaignEvent::CoverChanged { .. }
            | CampaignEvent::MemberJoined { .. }
            | CampaignEvent::InviteRegenerated { .. }
            | CampaignEvent::SettingsChanged { .. }
            | CampaignEvent::NoteCreated { .. }
            | CampaignEvent::NoteEdited { .. }
            | CampaignEvent::NoteAudienceChanged { .. }
            | CampaignEvent::NoteDeleted { .. }
            | CampaignEvent::SourcesChanged { .. } => {}
        }
        match n {
            NoteOp::Create { .. }
            | NoteOp::Edit { .. }
            | NoteOp::SetAudience { .. }
            | NoteOp::Delete { .. } => {}
        }
        match s {
            ServerMsg::HelloOk(_)
            | ServerMsg::HelloErr(_)
            | ServerMsg::Ack { .. }
            | ServerMsg::Reject { .. }
            | ServerMsg::Events { .. }
            | ServerMsg::Snapshot { .. }
            | ServerMsg::CampaignRemoved { .. }
            | ServerMsg::Presence { .. }
            | ServerMsg::PresenceChanged { .. }
            | ServerMsg::Pong { .. } => {}
        }
    }

    fn round_trip<T>(msg: &T)
    where
        T: Serialize + DeserializeOwned + PartialEq + core::fmt::Debug,
    {
        let frame = encode(msg).unwrap();
        let back: T = decode(&frame).unwrap();
        assert_eq!(&back, msg);
    }

    #[test]
    fn every_client_message_round_trips() {
        client_msgs().iter().for_each(round_trip);
    }

    #[test]
    fn every_server_message_round_trips() {
        server_msgs().iter().for_each(round_trip);
    }

    #[test]
    fn every_http_body_round_trips() {
        round_trip(&RegisterRequest {
            username: Username::new("bob".into()).unwrap(),
            display_name: DisplayName::new("Bob").unwrap(),
            password: Password::new("hunter2hunter2".into()).unwrap(),
        });
        round_trip(&LoginRequest {
            username: Username::new("bob".into()).unwrap(),
            password: Password::new("hunter2hunter2".into()).unwrap(),
        });
        round_trip(&AuthResponse {
            token: SessionToken([1; 32]),
            user: user(),
        });
        for e in [
            AuthError::UsernameTaken,
            AuthError::InvalidCredentials,
            AuthError::RateLimited,
            AuthError::UpdateRequired { server_version: 3 },
        ] {
            round_trip(&e);
        }
        round_trip(&vec![SourceInfo {
            id: "srd-5.2".into(),
            name: "System Reference Document 5.2.1".into(),
            publisher: "Wizards of the Coast".into(),
            system: beholder_core::compendium::GameSystem::Dnd5e,
            rules: beholder_core::compendium::RulesVersion::R2024,
            sha256: "ab".repeat(32),
            bytes: 200_759,
        }]);
        round_trip(&ImageUploaded {
            hash: ImageHash([5; 32]),
        });
        for e in [
            ImageError::TooLarge,
            ImageError::TooManyPixels,
            ImageError::Unsupported,
            ImageError::Unauthorized,
            ImageError::RateLimited,
        ] {
            round_trip(&e);
        }
    }

    #[test]
    fn small_frames_stay_uncompressed_and_tiny() {
        let ping = encode(&ClientMsg::Ping { t: 1_000_000 }).unwrap();
        assert_eq!(ping[0], 0);
        assert!(ping.len() <= 6, "ping is {} bytes", ping.len());
    }

    #[test]
    fn large_frames_are_compressed() {
        let msg = ServerMsg::Snapshot {
            seq: Seq(1),
            campaign: campaign(100),
        };
        let raw = postcard::to_stdvec(&msg).unwrap();
        assert!(raw.len() > COMPRESS_THRESHOLD);
        let frame = encode(&msg).unwrap();
        assert_eq!(frame[0], FLAG_ZSTD);
        assert!(frame.len() < raw.len());
        assert_eq!(decode::<ServerMsg>(&frame).unwrap(), msg);
    }

    #[test]
    fn bad_frames_are_rejected() {
        assert!(matches!(decode::<ClientMsg>(&[]), Err(CodecError::Empty)));
        assert!(matches!(
            decode::<ClientMsg>(&[0x80, 0]),
            Err(CodecError::UnknownFlags(0x80))
        ));
        assert!(matches!(
            decode::<ClientMsg>(&[0, 0xff, 0xff]),
            Err(CodecError::Postcard(_))
        ));
        let bomb = zstd::bulk::compress(&vec![0u8; MAX_FRAME_BYTES + 1], 3).unwrap();
        let mut frame = vec![FLAG_ZSTD];
        frame.extend_from_slice(&bomb);
        assert!(matches!(
            decode::<ClientMsg>(&frame),
            Err(CodecError::TooLarge)
        ));
    }

    #[test]
    fn invalid_values_fail_to_decode() {
        #[derive(Serialize)]
        enum FakeKind {
            CreateCampaign {
                campaign_id: CampaignId,
                title: String,
            },
        }
        #[derive(Serialize)]
        struct FakeCommand {
            id: CommandId,
            kind: FakeKind,
        }
        #[derive(Serialize)]
        enum FakeClientMsg {
            _Hello,
            Command(FakeCommand),
        }
        let frame = encode(&FakeClientMsg::Command(FakeCommand {
            id: cmd_id(1),
            kind: FakeKind::CreateCampaign {
                campaign_id: cid(1),
                title: "x".repeat(81),
            },
        }))
        .unwrap();
        assert!(decode::<ClientMsg>(&frame).is_err());
    }
}
