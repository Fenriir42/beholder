use beholder_core::{
    Campaign, CampaignEvent, CampaignId, CampaignOp, CampaignSettings, CampaignTitle,
    CombatVisibility, CommandId, DisplayName, ImageHash, InviteCode, LibraryId, Member, NoteId,
    Role, Seq, UnixMillis, User, UserId, ValidationError,
};
use beholder_protocol::{Command, CommandKind, EventEnvelope, RejectReason};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::db::DbError;
use crate::library;
use crate::notes;
use crate::permissions::{self, Viewer};

pub const DEDUP_WINDOW_MS: i64 = 7 * 24 * 3600 * 1000;

pub const COMMAND_RETENTION_MS: i64 = DEDUP_WINDOW_MS + 24 * 3600 * 1000;

pub const MAX_REPLAY: u64 = 1000;

pub const KEEP_EVENTS: u64 = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reply {
    Ack(Option<Seq>),
    Reject(RejectReason),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Handled {
    pub reply: Reply,
    pub campaign_id: Option<CampaignId>,
    pub events: Vec<EventEnvelope>,
    pub joined: Option<Role>,
    pub library_seq: Option<u64>,
}

impl Handled {
    fn reply_only(reply: Reply) -> Self {
        Self {
            reply,
            campaign_id: None,
            events: Vec::new(),
            joined: None,
            library_seq: None,
        }
    }
}

struct Executed {
    campaign_id: Option<CampaignId>,
    events: Vec<EventEnvelope>,
    joined: Option<Role>,
    library_seq: Option<u64>,
}

pub fn handle_command(
    conn: &mut Connection,
    actor: &User,
    cmd: &Command,
    now: UnixMillis,
) -> Result<Handled, DbError> {
    let tx = conn.transaction()?;
    let stored: Option<(Vec<u8>, Vec<u8>)> = tx
        .query_row(
            "SELECT user_id, outcome FROM commands WHERE id = ?1",
            [cmd.id.as_bytes()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((user, outcome)) = stored {
        if user != actor.id.as_bytes() {
            return Ok(Handled::reply_only(Reply::Reject(RejectReason::IdConflict)));
        }
        return Ok(Handled::reply_only(postcard::from_bytes(&outcome)?));
    }

    let result = match cmd.id.unix_millis() {
        None => Err(RejectReason::Invalid(ValidationError::Character)),
        Some(ms) if (ms as i64) < now.0 - DEDUP_WINDOW_MS => Err(RejectReason::Expired),
        Some(_) => execute(&tx, actor, cmd.id, &cmd.kind, now)?,
    };
    let handled = match result {
        Ok(ex) => Handled {
            reply: Reply::Ack(ex.library_seq.map(Seq).or(ex.events.last().map(|e| e.seq))),
            campaign_id: ex.campaign_id,
            events: ex.events,
            joined: ex.joined,
            library_seq: ex.library_seq,
        },
        Err(reason) => Handled::reply_only(Reply::Reject(reason)),
    };
    tx.execute(
        "INSERT INTO commands (id, user_id, outcome, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![
            cmd.id.as_bytes(),
            actor.id.as_bytes(),
            postcard::to_stdvec(&handled.reply)?,
            now.0
        ],
    )?;
    tx.commit()?;
    Ok(handled)
}

fn execute(
    tx: &Transaction<'_>,
    actor: &User,
    cmd_id: CommandId,
    kind: &CommandKind,
    now: UnixMillis,
) -> Result<Result<Executed, RejectReason>, DbError> {
    let log = Log {
        tx,
        actor: actor.id,
        cmd_id,
        now,
    };
    match kind {
        CommandKind::CreateCampaign { campaign_id, title } => {
            if campaign_exists(tx, *campaign_id)? {
                return Ok(Err(RejectReason::IdConflict));
            }
            let dm = Member {
                user_id: actor.id,
                display_name: actor.display_name.clone(),
                role: Role::Dm,
            };
            let created = log.emit(
                *campaign_id,
                CampaignEvent::Created {
                    title: title.clone(),
                    dm,
                },
            )?;
            let code = log.emit(
                *campaign_id,
                CampaignEvent::InviteRegenerated {
                    code: unused_invite_code(tx)?,
                },
            )?;
            Ok(Ok(Executed {
                campaign_id: Some(*campaign_id),
                events: vec![created, code],
                library_seq: None,
                joined: Some(Role::Dm),
            }))
        }
        CommandKind::JoinCampaign { code } => {
            let found: Option<Vec<u8>> = tx
                .query_row(
                    "SELECT id FROM campaigns WHERE invite_code = ?1",
                    [code.as_str()],
                    |r| r.get(0),
                )
                .optional()?;
            let Some(campaign_id) = found.map(|b| blob_id(&b).map(CampaignId)).transpose()? else {
                return Ok(Err(RejectReason::InvalidInviteCode));
            };
            if let Some(role) = role_of(tx, campaign_id, actor.id)? {
                return Ok(Ok(Executed {
                    campaign_id: Some(campaign_id),
                    events: Vec::new(),
                    library_seq: None,
                    joined: Some(role),
                }));
            }
            let member = Member {
                user_id: actor.id,
                display_name: actor.display_name.clone(),
                role: Role::Player,
            };
            let ev = log.emit(campaign_id, CampaignEvent::MemberJoined { member })?;
            Ok(Ok(Executed {
                campaign_id: Some(campaign_id),
                events: vec![ev],
                library_seq: None,
                joined: Some(Role::Player),
            }))
        }
        CommandKind::Campaign { campaign_id, op } => {
            let Some(role) = role_of(tx, *campaign_id, actor.id)? else {
                return Ok(Err(RejectReason::NotFound));
            };
            if !permissions::can_apply(role, op) {
                return Ok(Err(RejectReason::Forbidden));
            }
            if let CampaignOp::SetCover { cover: Some(hash) } = op
                && !image_exists(tx, *hash)?
            {
                return Ok(Err(RejectReason::UnknownImage));
            }
            let event = match op.predicted_event() {
                Some(e) => e,
                None => CampaignEvent::InviteRegenerated {
                    code: unused_invite_code(tx)?,
                },
            };
            let ev = log.emit(*campaign_id, event)?;
            Ok(Ok(Executed {
                campaign_id: Some(*campaign_id),
                events: vec![ev],
                library_seq: None,
                joined: None,
            }))
        }
        CommandKind::Library { op } => Ok(library::decide(tx, actor.id, cmd_id, op, now)?.map(
            |seq| Executed {
                campaign_id: None,
                events: Vec::new(),
                joined: None,
                library_seq: Some(seq),
            },
        )),
        CommandKind::Note { campaign_id, op } => {
            let Some(role) = role_of(tx, *campaign_id, actor.id)? else {
                return Ok(Err(RejectReason::NotFound));
            };
            let viewer = Viewer {
                user: actor.id,
                role,
            };
            let settings = load_settings(tx, *campaign_id)?;
            let decided = notes::decide(tx, viewer, *campaign_id, &settings, op, NoteId(cmd_id.0))?;
            let event = match decided {
                Ok(e) => e,
                Err(reason) => return Ok(Err(reason)),
            };
            let events = match event {
                Some(e) => vec![log.emit(*campaign_id, e)?],
                None => Vec::new(),
            };
            Ok(Ok(Executed {
                campaign_id: Some(*campaign_id),
                events,
                library_seq: None,
                joined: None,
            }))
        }
    }
}

pub fn load_settings(conn: &Connection, id: CampaignId) -> Result<CampaignSettings, DbError> {
    let (dm_read, vis): (bool, i64) = conn.query_row(
        "SELECT dm_can_read_private_notes, combat_visibility FROM campaigns WHERE id = ?1",
        [id.as_bytes()],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok(CampaignSettings {
        dm_can_read_private_notes: dm_read,
        combat_visibility: int_to_visibility(vis)?,
    })
}

struct Log<'a> {
    tx: &'a Transaction<'a>,
    actor: UserId,
    cmd_id: CommandId,
    now: UnixMillis,
}

impl Log<'_> {
    fn emit(
        &self,
        campaign_id: CampaignId,
        event: CampaignEvent,
    ) -> Result<EventEnvelope, DbError> {
        let tx = self.tx;
        let id = campaign_id.as_bytes();
        if let CampaignEvent::Created { title, .. } = &event {
            tx.execute(
                "INSERT INTO campaigns (id, title, created_at) VALUES (?1, ?2, ?3)",
                params![id, title.as_str(), self.now.0],
            )?;
        }
        let seq: u64 = tx.query_row(
            "UPDATE campaigns SET head_seq = head_seq + 1 WHERE id = ?1 RETURNING head_seq",
            [id],
            |r| r.get(0),
        )?;
        tx.execute(
            "INSERT INTO events (campaign_id, seq, ts, actor, command_id, body)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                seq,
                self.now.0,
                self.actor.as_bytes(),
                self.cmd_id.as_bytes(),
                postcard::to_stdvec(&event)?
            ],
        )?;
        project(tx, campaign_id, Seq(seq), &event)?;
        Ok(EventEnvelope {
            seq: Seq(seq),
            ts: self.now,
            actor: self.actor,
            event,
        })
    }
}

fn project(
    tx: &Transaction<'_>,
    campaign_id: CampaignId,
    seq: Seq,
    event: &CampaignEvent,
) -> Result<(), DbError> {
    let id = campaign_id.as_bytes();
    match event {
        CampaignEvent::Created { dm: member, .. } | CampaignEvent::MemberJoined { member } => {
            tx.execute(
                "INSERT INTO memberships (campaign_id, user_id, role, display_name, joined_seq)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (campaign_id, user_id)
                 DO UPDATE SET role = excluded.role, display_name = excluded.display_name",
                params![
                    id,
                    member.user_id.as_bytes(),
                    role_to_int(member.role),
                    member.display_name.as_str(),
                    seq.0
                ],
            )?;
        }
        CampaignEvent::Renamed { title } => {
            tx.execute(
                "UPDATE campaigns SET title = ?2 WHERE id = ?1",
                params![id, title.as_str()],
            )?;
        }
        CampaignEvent::CoverChanged { cover } => {
            tx.execute(
                "UPDATE campaigns SET cover_hash = ?2 WHERE id = ?1",
                params![id, cover.as_ref().map(|h| h.0.as_slice())],
            )?;
        }
        CampaignEvent::InviteRegenerated { code } => {
            tx.execute(
                "UPDATE campaigns SET invite_code = ?2 WHERE id = ?1",
                params![id, code.as_str()],
            )?;
        }
        CampaignEvent::SettingsChanged { settings } => {
            tx.execute(
                "UPDATE campaigns SET dm_can_read_private_notes = ?2, combat_visibility = ?3 WHERE id = ?1",
                params![
                    id,
                    settings.dm_can_read_private_notes,
                    visibility_to_int(settings.combat_visibility)
                ],
            )?;
        }
        CampaignEvent::NoteCreated { .. }
        | CampaignEvent::NoteEdited { .. }
        | CampaignEvent::NoteAudienceChanged { .. }
        | CampaignEvent::NoteDeleted { .. } => notes::project(tx, campaign_id, event)?,
        CampaignEvent::SourcesChanged { sources } => {
            tx.execute(
                "UPDATE campaigns SET sources = ?2 WHERE id = ?1",
                params![id, postcard::to_stdvec(sources)?],
            )?;
        }
        CampaignEvent::EntryHidden { id: entry, hidden } => {
            let stored: Option<Vec<u8>> =
                tx.query_row("SELECT hidden FROM campaigns WHERE id = ?1", [id], |r| {
                    r.get(0)
                })?;
            let mut list: Vec<LibraryId> = stored
                .map(|b| postcard::from_bytes(&b))
                .transpose()?
                .unwrap_or_default();
            beholder_core::campaign::set_hidden(&mut list, *entry, *hidden);
            tx.execute(
                "UPDATE campaigns SET hidden = ?2 WHERE id = ?1",
                params![id, postcard::to_stdvec(&list)?],
            )?;
        }
    }
    Ok(())
}

pub fn load_campaign(
    conn: &Connection,
    id: CampaignId,
) -> Result<Option<(Seq, Campaign)>, DbError> {
    let row = conn
        .query_row(
            "SELECT title, cover_hash, invite_code, dm_can_read_private_notes, combat_visibility, head_seq, sources, hidden
             FROM campaigns WHERE id = ?1",
            [id.as_bytes()],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<Vec<u8>>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, bool>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, u64>(5)?,                    r.get::<_, Option<Vec<u8>>>(6)?,
                    r.get::<_, Option<Vec<u8>>>(7)?,
                ))
            },
        )
        .optional()?;
    let Some((title, cover, code, dm_read, vis, head, sources, hidden)) = row else {
        return Ok(None);
    };
    let mut stmt = conn.prepare_cached(
        "SELECT user_id, display_name, role FROM memberships WHERE campaign_id = ?1 ORDER BY joined_seq",
    )?;
    let members = stmt
        .query_map([id.as_bytes()], |r| {
            Ok((
                r.get::<_, Vec<u8>>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?
        .map(|row| {
            let (user, name, role) = row?;
            Ok(Member {
                user_id: UserId(blob_id(&user)?),
                display_name: DisplayName::new(&name)
                    .map_err(|_| DbError::Corrupt("display_name"))?,
                role: int_to_role(role)?,
            })
        })
        .collect::<Result<Vec<_>, DbError>>()?;
    let campaign = Campaign {
        id,
        title: CampaignTitle::new(&title).map_err(|_| DbError::Corrupt("title"))?,
        cover: cover
            .map(|b| <[u8; 32]>::try_from(b.as_slice()).map(ImageHash))
            .transpose()
            .map_err(|_| DbError::Corrupt("cover_hash"))?,
        settings: CampaignSettings {
            dm_can_read_private_notes: dm_read,
            combat_visibility: int_to_visibility(vis)?,
        },
        invite_code: code
            .map(|c| InviteCode::parse(&c))
            .transpose()
            .map_err(|_| DbError::Corrupt("invite_code"))?,
        members,
        notes: notes::campaign_notes(conn, id)?,
        sources: match sources {
            Some(b) => postcard::from_bytes(&b)?,
            None => vec![
                beholder_core::compendium::SRD.to_owned(),
                beholder_core::compendium::LIBRARY.to_owned(),
            ],
        },
        hidden: hidden
            .map(|b| postcard::from_bytes(&b))
            .transpose()?
            .unwrap_or_default(),
    };
    Ok(Some((Seq(head), campaign)))
}

#[derive(Debug, PartialEq, Eq)]
pub enum CatchUp {
    Events(Vec<EventEnvelope>),
    Snapshot,
}

pub fn catch_up(conn: &Connection, id: CampaignId, after: Seq) -> Result<CatchUp, DbError> {
    let (head, compacted): (u64, u64) = conn.query_row(
        "SELECT head_seq, compacted_seq FROM campaigns WHERE id = ?1",
        [id.as_bytes()],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if after.0 == 0 || after.0 < compacted || after.0 > head || head - after.0 > MAX_REPLAY {
        return Ok(CatchUp::Snapshot);
    }
    let mut stmt = conn.prepare_cached(
        "SELECT seq, ts, actor, body FROM events WHERE campaign_id = ?1 AND seq > ?2 ORDER BY seq",
    )?;
    let events = stmt
        .query_map(params![id.as_bytes(), after.0], |r| {
            Ok((
                r.get::<_, u64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, Vec<u8>>(2)?,
                r.get::<_, Vec<u8>>(3)?,
            ))
        })?
        .map(|row| {
            let (seq, ts, actor, body) = row?;
            Ok(EventEnvelope {
                seq: Seq(seq),
                ts: UnixMillis(ts),
                actor: UserId(blob_id(&actor)?),
                event: postcard::from_bytes(&body)?,
            })
        })
        .collect::<Result<Vec<_>, DbError>>()?;
    // Replay is filtered with today's audiences and settings, which is only right if none changed in range.
    if events.iter().any(|e| {
        matches!(
            e.event,
            CampaignEvent::NoteAudienceChanged { .. } | CampaignEvent::SettingsChanged { .. }
        )
    }) {
        return Ok(CatchUp::Snapshot);
    }
    Ok(CatchUp::Events(events))
}

pub fn memberships_of(conn: &Connection, user: UserId) -> Result<Vec<(CampaignId, Role)>, DbError> {
    let mut stmt =
        conn.prepare_cached("SELECT campaign_id, role FROM memberships WHERE user_id = ?1")?;
    stmt.query_map([user.as_bytes()], |r| {
        Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, i64>(1)?))
    })?
    .map(|row| {
        let (id, role) = row?;
        Ok((CampaignId(blob_id(&id)?), int_to_role(role)?))
    })
    .collect()
}

pub fn compact(conn: &mut Connection, now: UnixMillis) -> Result<(), DbError> {
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE campaigns SET compacted_seq = head_seq - ?1 WHERE head_seq - compacted_seq > ?1 * 2",
        [KEEP_EVENTS],
    )?;
    tx.execute(
        "DELETE FROM events WHERE seq <= (SELECT compacted_seq FROM campaigns c WHERE c.id = events.campaign_id)",
        [],
    )?;
    tx.execute(
        "DELETE FROM commands WHERE created_at < ?1",
        [now.0 - COMMAND_RETENTION_MS],
    )?;
    tx.commit()?;
    Ok(())
}

fn campaign_exists(tx: &Transaction<'_>, id: CampaignId) -> Result<bool, DbError> {
    Ok(tx
        .query_row(
            "SELECT 1 FROM campaigns WHERE id = ?1",
            [id.as_bytes()],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn image_exists(tx: &Transaction<'_>, hash: ImageHash) -> Result<bool, DbError> {
    Ok(tx
        .query_row(
            "SELECT 1 FROM images WHERE hash = ?1",
            [hash.0.as_slice()],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

pub(crate) fn role_of(
    tx: &Transaction<'_>,
    campaign: CampaignId,
    user: UserId,
) -> Result<Option<Role>, DbError> {
    tx.query_row(
        "SELECT role FROM memberships WHERE campaign_id = ?1 AND user_id = ?2",
        [campaign.as_bytes(), user.as_bytes()],
        |r| r.get::<_, i64>(0),
    )
    .optional()?
    .map(int_to_role)
    .transpose()
}

fn unused_invite_code(tx: &Transaction<'_>) -> Result<InviteCode, DbError> {
    // 40 random bits: a collision is rare, and the serialized connection rules out races.
    loop {
        let code = InviteCode::from_random(crate::random_bytes()?);
        let taken = tx
            .query_row(
                "SELECT 1 FROM campaigns WHERE invite_code = ?1",
                [code.as_str()],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !taken {
            return Ok(code);
        }
    }
}

pub(crate) fn blob_id(b: &[u8]) -> Result<uuid::Uuid, DbError> {
    uuid::Uuid::from_slice(b).map_err(|_| DbError::Corrupt("id"))
}

fn role_to_int(role: Role) -> i64 {
    match role {
        Role::Dm => 0,
        Role::Player => 1,
    }
}

fn int_to_role(v: i64) -> Result<Role, DbError> {
    match v {
        0 => Ok(Role::Dm),
        1 => Ok(Role::Player),
        _ => Err(DbError::Corrupt("role")),
    }
}

fn visibility_to_int(v: CombatVisibility) -> i64 {
    match v {
        CombatVisibility::Exact => 0,
        CombatVisibility::Vague => 1,
        CombatVisibility::Hidden => 2,
    }
}

fn int_to_visibility(v: i64) -> Result<CombatVisibility, DbError> {
    match v {
        0 => Ok(CombatVisibility::Exact),
        1 => Ok(CombatVisibility::Vague),
        2 => Ok(CombatVisibility::Hidden),
        _ => Err(DbError::Corrupt("combat_visibility")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use beholder_core::{Audience, NoteBody, NoteOp, NoteTitle, Username};
    use uuid::Uuid;

    fn setup() -> (Connection, User, User) {
        let mut c = Db::open_in_memory().unwrap().into_connection();
        let mut mk = |name: &str| {
            crate::auth::create_user(
                &mut c,
                &Username::new(name.into()).unwrap(),
                &DisplayName::new(name).unwrap(),
                "phc",
                UnixMillis(0),
            )
            .unwrap()
            .unwrap()
        };
        let (dm, player) = (mk("dmaster"), mk("player"));
        (c, dm, player)
    }

    fn cmd(kind: CommandKind) -> Command {
        Command {
            id: CommandId(Uuid::now_v7()),
            kind,
        }
    }

    fn run(c: &mut Connection, actor: &User, kind: CommandKind) -> Handled {
        handle_command(c, actor, &cmd(kind), crate::now()).unwrap()
    }

    fn create(c: &mut Connection, dm: &User) -> CampaignId {
        let id = CampaignId(Uuid::now_v7());
        let h = run(
            c,
            dm,
            CommandKind::CreateCampaign {
                campaign_id: id,
                title: CampaignTitle::new("Start").unwrap(),
            },
        );
        assert_eq!(h.reply, Reply::Ack(Some(Seq(2))));
        assert_eq!(h.joined, Some(Role::Dm));
        id
    }

    fn rename(id: CampaignId, t: &str) -> CommandKind {
        CommandKind::Campaign {
            campaign_id: id,
            op: CampaignOp::Rename {
                title: CampaignTitle::new(t).unwrap(),
            },
        }
    }

    #[test]
    fn projection_equals_event_fold() {
        let (mut c, dm, player) = setup();
        let id = create(&mut c, &dm);
        let (_, state) = load_campaign(&c, id).unwrap().unwrap();
        let code = state.invite_code.unwrap();
        run(&mut c, &player, CommandKind::JoinCampaign { code });
        run(&mut c, &dm, rename(id, "Renamed"));
        run(
            &mut c,
            &dm,
            CommandKind::Campaign {
                campaign_id: id,
                op: CampaignOp::UpdateSettings(CampaignSettings {
                    dm_can_read_private_notes: true,
                    combat_visibility: CombatVisibility::Hidden,
                }),
            },
        );
        run(
            &mut c,
            &dm,
            CommandKind::Campaign {
                campaign_id: id,
                op: CampaignOp::RegenerateInvite,
            },
        );
        run(
            &mut c,
            &dm,
            CommandKind::Campaign {
                campaign_id: id,
                op: CampaignOp::SetSources {
                    sources: vec!["tob3".into(), "srd-5.2".into()],
                },
            },
        );
        let hidden = LibraryId(Uuid::now_v7());
        for h in [true, true, false, true] {
            run(
                &mut c,
                &dm,
                CommandKind::Campaign {
                    campaign_id: id,
                    op: CampaignOp::HideEntry {
                        id: hidden,
                        hidden: h,
                    },
                },
            );
        }
        let (party, secret, gone) = (nid(), nid(), nid());
        for (who, op) in [
            (&player, create_note(party, Audience::Party)),
            (&dm, edit_note(party, 1, "edited by the DM")),
            (&dm, create_note(secret, Audience::Private)),
            (
                &dm,
                NoteOp::SetAudience {
                    id: secret,
                    audience: Audience::Party,
                },
            ),
            (&player, create_note(gone, Audience::Private)),
            (&player, NoteOp::Delete { id: gone }),
            (&player, edit_note(party, 1, "stale edit")),
        ] {
            let h = run(&mut c, who, note_cmd(id, op));
            assert!(matches!(h.reply, Reply::Ack(Some(_))), "{h:?}");
        }

        let (head, projected) = load_campaign(&c, id).unwrap().unwrap();
        let events = catch_up_all(&c, id);
        let CampaignEvent::Created { title, dm: first } = &events[0].event else {
            panic!("first event must be Created");
        };
        let mut folded = Campaign::new(id, title.clone(), first.clone());
        for e in &events {
            folded.apply(&e.event);
        }
        assert_eq!(Some(head), events.last().map(|e| e.seq));
        assert_eq!(projected, folded);
        assert_ne!(projected.invite_code, Some(code));
        assert_eq!(projected.notes.len(), 3, "two notes plus one conflict copy");
        assert_eq!(projected.sources, ["tob3", "srd-5.2"]);
        assert_eq!(projected.hidden, [hidden]);
    }

    fn nid() -> NoteId {
        NoteId(Uuid::now_v7())
    }

    fn note_cmd(campaign_id: CampaignId, op: NoteOp) -> CommandKind {
        CommandKind::Note { campaign_id, op }
    }

    fn create_note(id: NoteId, audience: Audience) -> NoteOp {
        NoteOp::Create {
            id,
            title: NoteTitle::new("Clue").unwrap(),
            body: NoteBody::new("the priest".into()).unwrap(),
            audience,
        }
    }

    fn edit_note(id: NoteId, base_version: u32, body: &str) -> NoteOp {
        NoteOp::Edit {
            id,
            base_version,
            title: NoteTitle::new("Clue").unwrap(),
            body: NoteBody::new(body.into()).unwrap(),
        }
    }

    #[test]
    fn note_commands() {
        let (mut c, dm, player) = setup();
        let id = create(&mut c, &dm);
        let (_, state) = load_campaign(&c, id).unwrap().unwrap();
        run(
            &mut c,
            &player,
            CommandKind::JoinCampaign {
                code: state.invite_code.unwrap(),
            },
        );
        let mut reply = |who: &User, op| run(&mut c, who, note_cmd(id, op)).reply;
        let dm_only = nid();
        assert!(matches!(
            reply(&dm, create_note(dm_only, Audience::Private)),
            Reply::Ack(Some(_))
        ));
        assert_eq!(
            reply(&dm, create_note(dm_only, Audience::Party)),
            Reply::Reject(RejectReason::IdConflict)
        );
        assert_eq!(
            reply(&player, edit_note(dm_only, 1, "peek")),
            Reply::Reject(RejectReason::NotFound),
            "a hidden note is indistinguishable from a missing one"
        );
        assert_eq!(
            reply(&player, NoteOp::Delete { id: dm_only }),
            Reply::Reject(RejectReason::NotFound)
        );

        let shared = nid();
        assert!(matches!(
            reply(&dm, create_note(shared, Audience::Party)),
            Reply::Ack(Some(_))
        ));
        assert!(matches!(
            reply(&player, edit_note(shared, 1, "mine now")),
            Reply::Ack(Some(_))
        ));
        assert_eq!(
            reply(
                &player,
                NoteOp::SetAudience {
                    id: shared,
                    audience: Audience::Private
                }
            ),
            Reply::Reject(RejectReason::Forbidden),
            "only the author changes the audience"
        );
        assert_eq!(
            reply(&player, NoteOp::Delete { id: shared }),
            Reply::Reject(RejectReason::Forbidden)
        );
        assert_eq!(
            reply(
                &dm,
                NoteOp::SetAudience {
                    id: shared,
                    audience: Audience::Party
                }
            ),
            Reply::Ack(None),
            "an unchanged audience emits nothing"
        );
        let stranger = UserId(Uuid::now_v7());
        assert_eq!(
            reply(
                &dm,
                NoteOp::SetAudience {
                    id: shared,
                    audience: Audience::Users(vec![stranger])
                }
            ),
            Reply::Reject(RejectReason::NotFound),
            "audiences only name members"
        );

        let h = run(&mut c, &dm, note_cmd(id, edit_note(shared, 1, "stale")));
        let [
            EventEnvelope {
                event: CampaignEvent::NoteCreated { note: copy },
                ..
            },
        ] = h.events.as_slice()
        else {
            panic!("a stale edit creates a conflict copy: {h:?}");
        };
        assert_eq!(copy.conflict_of, Some(shared));
        assert_eq!(copy.audience, Audience::Private);
        assert_eq!(copy.author, dm.id);
        let (_, state) = load_campaign(&c, id).unwrap().unwrap();
        assert_eq!(
            state.note(shared).map(|n| n.body.as_str()),
            Some("mine now"),
            "the original keeps the newer text"
        );
    }

    fn draft(id: LibraryId, name: &str) -> beholder_core::library::LibraryDraft {
        beholder_core::library::LibraryDraft {
            id,
            name: NoteTitle::new(name).unwrap(),
            facets: beholder_core::compendium::Facets::Condition,
            text: NoteBody::new("Can't act.".into()).unwrap(),
            book: None,
            rules: beholder_core::compendium::RulesVersion::R2024,
        }
    }

    fn save(id: LibraryId, name: &str, base_version: u32) -> CommandKind {
        CommandKind::Library {
            op: beholder_core::library::LibraryOp::Save {
                draft: draft(id, name),
                base_version,
            },
        }
    }

    #[test]
    fn library_commands() {
        let (mut c, dm, player) = setup();
        let other = crate::auth::create_user(
            &mut c,
            &Username::new("other".into()).unwrap(),
            &DisplayName::new("Other").unwrap(),
            "phc",
            UnixMillis(0),
        )
        .unwrap()
        .unwrap();
        create(&mut c, &dm);
        let id = LibraryId(Uuid::now_v7());
        let reply = |c: &mut Connection, who: &User, kind| run(c, who, kind).reply;

        assert_eq!(
            reply(&mut c, &player, save(id, "Dazed", 0)),
            Reply::Ack(Some(Seq(1)))
        );
        assert_eq!(
            reply(&mut c, &player, save(id, "Again", 0)),
            Reply::Reject(RejectReason::IdConflict)
        );
        assert_eq!(
            reply(&mut c, &other, save(id, "Mine now", 1)),
            Reply::Reject(RejectReason::Forbidden),
            "another player can't edit someone's entry"
        );
        assert!(matches!(
            reply(&mut c, &player, save(id, "Dazed v2", 1)),
            Reply::Ack(Some(_))
        ));
        assert_eq!(
            reply(&mut c, &player, save(id, "Stale", 1)),
            Reply::Reject(RejectReason::Stale)
        );
        assert!(
            matches!(
                reply(&mut c, &dm, save(id, "Tidied", 2)),
                Reply::Ack(Some(_))
            ),
            "a DM of any campaign can edit"
        );
        let mut bad = draft(LibraryId(Uuid::now_v7()), "Bad");
        bad.facets = beholder_core::compendium::Facets::Feature {
            class: "Wizard".into(),
            level: 0,
        };
        assert!(matches!(
            reply(
                &mut c,
                &player,
                CommandKind::Library {
                    op: beholder_core::library::LibraryOp::Save {
                        draft: bad,
                        base_version: 0
                    }
                }
            ),
            Reply::Reject(RejectReason::Invalid(_))
        ));

        let b = crate::library::bundle(&c).unwrap();
        assert_eq!(b.seq, 3);
        assert_eq!(b.entries.len(), 1);
        assert_eq!(b.entries[0].name, "Tidied");
        let ch = crate::library::changes(&c, 1).unwrap();
        assert_eq!(ch.head, 3);
        assert_eq!(ch.changes.map(|v| v.len()), Some(2));
        assert_eq!(
            crate::library::changes(&c, 9).unwrap().changes,
            None,
            "ahead of the server: reload"
        );

        let delete = CommandKind::Library {
            op: beholder_core::library::LibraryOp::Delete { id },
        };
        assert_eq!(
            reply(&mut c, &other, delete.clone()),
            Reply::Reject(RejectReason::Forbidden)
        );
        assert!(matches!(
            reply(&mut c, &player, delete.clone()),
            Reply::Ack(Some(_))
        ));
        assert_eq!(
            reply(&mut c, &player, delete),
            Reply::Reject(RejectReason::NotFound)
        );
        assert_eq!(
            reply(&mut c, &player, save(id, "Back", 0)),
            Reply::Reject(RejectReason::NotFound),
            "a deleted id isn't reused"
        );
        assert!(crate::library::bundle(&c).unwrap().entries.is_empty());
    }

    #[test]
    fn library_batches_are_all_or_nothing_and_stream_to_disk() {
        let (mut c, _, player) = setup();
        let many = |drafts| CommandKind::Library {
            op: beholder_core::library::LibraryOp::SaveMany { drafts },
        };
        let ids: Vec<LibraryId> = (0..3).map(|_| LibraryId(Uuid::now_v7())).collect();
        let mut drafts: Vec<_> = ids.iter().map(|id| draft(*id, "Batch entry")).collect();
        drafts[2].facets = beholder_core::compendium::Facets::Feature {
            class: "Wizard".into(),
            level: 0,
        };
        assert!(matches!(
            run(&mut c, &player, many(drafts.clone())).reply,
            Reply::Reject(RejectReason::Invalid(_))
        ));
        assert!(
            crate::library::bundle(&c).unwrap().entries.is_empty(),
            "nothing was saved"
        );
        drafts[2] = draft(ids[1], "Duplicate id");
        assert_eq!(
            run(&mut c, &player, many(drafts.clone())).reply,
            Reply::Reject(RejectReason::IdConflict)
        );
        drafts[2] = draft(ids[2], "Third");
        assert_eq!(
            run(&mut c, &player, many(drafts.clone())).reply,
            Reply::Ack(Some(Seq(3)))
        );
        assert_eq!(
            run(&mut c, &player, many(vec![draft(ids[0], "Again")])).reply,
            Reply::Reject(RejectReason::IdConflict),
            "a batch only creates"
        );
        let oversized = (0..=beholder_core::library::MAX_BATCH)
            .map(|_| draft(LibraryId(Uuid::now_v7()), "Too many"))
            .collect();
        assert!(matches!(
            run(&mut c, &player, many(oversized)).reply,
            Reply::Reject(RejectReason::Invalid(_))
        ));

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.bundle");
        let w = crate::library::write_bundle(&c, &path).unwrap();
        let file = std::fs::read(&path).unwrap();
        assert_eq!(w.bytes, file.len() as u64);
        assert_eq!(w.sha256, crate::compendium::sha256_hex(&file));
        assert_eq!(w.seq, 3);
        let raw = zstd::bulk::decompress(&file, 64 << 20).unwrap();
        assert_eq!(
            raw,
            postcard::to_stdvec(&crate::library::bundle(&c).unwrap()).unwrap(),
            "the streamed bundle is byte-identical to the in-memory one"
        );
    }

    #[test]
    fn library_upserts_respect_rights_and_versions() {
        let (mut c, dm, player) = setup();
        create(&mut c, &dm);
        let upsert = |items| CommandKind::Library {
            op: beholder_core::library::LibraryOp::UpsertMany { items },
        };
        let (a, b) = (LibraryId(Uuid::now_v7()), LibraryId(Uuid::now_v7()));
        assert_eq!(
            run(&mut c, &dm, upsert(vec![(draft(a, "DM entry"), 0)])).reply,
            Reply::Ack(Some(Seq(1)))
        );
        assert_eq!(
            run(
                &mut c,
                &player,
                upsert(vec![(draft(b, "New"), 0), (draft(a, "Hijack"), 1)])
            )
            .reply,
            Reply::Reject(RejectReason::Forbidden),
            "a player can't replace a DM's entry, and the batch writes nothing"
        );
        assert!(
            !crate::library::bundle(&c)
                .unwrap()
                .entries
                .iter()
                .any(|e| e.name == "New"),
            "all or nothing"
        );
        assert_eq!(
            run(&mut c, &dm, upsert(vec![(draft(a, "Stale"), 7)])).reply,
            Reply::Reject(RejectReason::Stale)
        );
        assert_eq!(
            run(
                &mut c,
                &dm,
                upsert(vec![(draft(a, "Replaced"), 1), (draft(b, "Added"), 0)])
            )
            .reply,
            Reply::Ack(Some(Seq(3)))
        );
        let names: Vec<String> = crate::library::bundle(&c)
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert!(names.contains(&"Replaced".to_owned()) && names.contains(&"Added".to_owned()));
    }

    #[test]
    fn imports_undo_in_one_action_and_keep_later_edits() {
        use beholder_core::ImportId;
        use beholder_core::library::LibraryOp;
        let (mut c, dm, player) = setup();
        create(&mut c, &dm);
        let lib = |op| CommandKind::Library { op };
        let (old, fresh, edited) = (
            LibraryId(Uuid::now_v7()),
            LibraryId(Uuid::now_v7()),
            LibraryId(Uuid::now_v7()),
        );
        run(
            &mut c,
            &player,
            lib(LibraryOp::Save {
                draft: draft(old, "Original"),
                base_version: 0,
            }),
        );
        let import = ImportId(Uuid::now_v7());
        let batch = |items| lib(LibraryOp::ImportMany { import, items });
        run(
            &mut c,
            &player,
            batch(vec![
                (draft(old, "Imported"), 1),
                (draft(fresh, "Fresh"), 0),
            ]),
        );
        run(
            &mut c,
            &player,
            batch(vec![(draft(edited, "Will be edited"), 0)]),
        );
        run(
            &mut c,
            &dm,
            lib(LibraryOp::Save {
                draft: draft(edited, "Edited later"),
                base_version: 1,
            }),
        );
        let other = ImportId(Uuid::now_v7());
        assert_eq!(
            run(
                &mut c,
                &dm,
                lib(LibraryOp::ImportMany {
                    import,
                    items: vec![(draft(LibraryId(Uuid::now_v7()), "Sneaky"), 0)],
                })
            )
            .reply,
            Reply::Reject(RejectReason::Forbidden),
            "an import belongs to the user who started it"
        );
        assert_eq!(
            run(&mut c, &dm, lib(LibraryOp::UndoImport { import: other })).reply,
            Reply::Reject(RejectReason::NotFound)
        );
        let list = crate::library::imports(&c).unwrap();
        assert_eq!((list.len(), list[0].entries, list[0].undone), (1, 3, false));
        assert!(matches!(
            run(&mut c, &player, lib(LibraryOp::UndoImport { import })).reply,
            Reply::Ack(Some(_))
        ));
        let names: Vec<String> = crate::library::bundle(&c)
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert!(names.contains(&"Original".to_owned()), "{names:?}");
        assert!(!names.contains(&"Fresh".to_owned()), "{names:?}");
        assert!(
            names.contains(&"Edited later".to_owned()),
            "later edits are kept: {names:?}"
        );
        let list = crate::library::imports(&c).unwrap();
        assert_eq!((list[0].undone, list[0].kept), (true, 1));
        assert_eq!(
            run(
                &mut c,
                &player,
                batch(vec![(draft(LibraryId(Uuid::now_v7()), "Late"), 0)])
            )
            .reply,
            Reply::Reject(RejectReason::Stale),
            "an undone import takes no more batches"
        );
    }

    #[test]
    fn catch_up_snapshots_across_visibility_changes() {
        let (mut c, dm, _) = setup();
        let id = create(&mut c, &dm);
        let n = nid();
        run(&mut c, &dm, note_cmd(id, create_note(n, Audience::Private)));
        run(&mut c, &dm, note_cmd(id, edit_note(n, 1, "v2")));
        assert!(matches!(catch_up(&c, id, Seq(2)).unwrap(), CatchUp::Events(e) if e.len() == 2));
        run(
            &mut c,
            &dm,
            note_cmd(
                id,
                NoteOp::SetAudience {
                    id: n,
                    audience: Audience::Party,
                },
            ),
        );
        assert_eq!(catch_up(&c, id, Seq(2)).unwrap(), CatchUp::Snapshot);
        assert!(matches!(catch_up(&c, id, Seq(5)).unwrap(), CatchUp::Events(e) if e.is_empty()));
    }

    fn catch_up_all(c: &Connection, id: CampaignId) -> Vec<EventEnvelope> {
        let mut stmt = c
            .prepare("SELECT seq, ts, actor, body FROM events WHERE campaign_id = ?1 ORDER BY seq")
            .unwrap();
        stmt.query_map([id.as_bytes()], |r| {
            Ok(EventEnvelope {
                seq: Seq(r.get(0)?),
                ts: UnixMillis(r.get(1)?),
                actor: UserId(blob_id(&r.get::<_, Vec<u8>>(2)?).unwrap()),
                event: postcard::from_bytes(&r.get::<_, Vec<u8>>(3)?).unwrap(),
            })
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
    }

    #[test]
    fn duplicate_command_replays_its_reply_without_new_events() {
        let (mut c, dm, player) = setup();
        let id = create(&mut c, &dm);
        let once = cmd(rename(id, "Once"));
        let first = handle_command(&mut c, &dm, &once, crate::now()).unwrap();
        let again = handle_command(&mut c, &dm, &once, crate::now()).unwrap();
        assert_eq!(first.reply, again.reply);
        assert!(again.events.is_empty());
        assert_eq!(load_campaign(&c, id).unwrap().unwrap().0, Seq(3));
        let stolen = handle_command(&mut c, &player, &once, crate::now()).unwrap();
        assert_eq!(stolen.reply, Reply::Reject(RejectReason::IdConflict));
    }

    #[test]
    fn rejections() {
        let (mut c, dm, player) = setup();
        let id = create(&mut c, &dm);
        let h = run(&mut c, &player, rename(id, "Nope"));
        assert_eq!(h.reply, Reply::Reject(RejectReason::NotFound), "non-member");
        let code = load_campaign(&c, id)
            .unwrap()
            .unwrap()
            .1
            .invite_code
            .unwrap();
        run(&mut c, &player, CommandKind::JoinCampaign { code });
        let h = run(&mut c, &player, rename(id, "Nope"));
        assert_eq!(h.reply, Reply::Reject(RejectReason::Forbidden));
        let h = run(
            &mut c,
            &dm,
            CommandKind::Campaign {
                campaign_id: id,
                op: CampaignOp::SetCover {
                    cover: Some(ImageHash([1; 32])),
                },
            },
        );
        assert_eq!(h.reply, Reply::Reject(RejectReason::UnknownImage));
        let h = run(
            &mut c,
            &dm,
            CommandKind::CreateCampaign {
                campaign_id: id,
                title: CampaignTitle::new("Again").unwrap(),
            },
        );
        assert_eq!(h.reply, Reply::Reject(RejectReason::IdConflict));
        let old = Command {
            id: CommandId(Uuid::from_bytes([
                0x01, 0x70, 0, 0, 0, 0, 0x70, 0, 0x80, 0, 0, 0, 0, 0, 0, 1,
            ])),
            kind: rename(id, "Late"),
        };
        let h = handle_command(&mut c, &dm, &old, crate::now()).unwrap();
        assert_eq!(h.reply, Reply::Reject(RejectReason::Expired));
    }

    #[test]
    fn catch_up_switches_to_snapshot() {
        let (mut c, dm, _) = setup();
        let id = create(&mut c, &dm);
        assert_eq!(catch_up(&c, id, Seq(0)).unwrap(), CatchUp::Snapshot);
        assert_eq!(catch_up(&c, id, Seq(2)).unwrap(), CatchUp::Events(vec![]));
        assert_eq!(
            catch_up(&c, id, Seq(9)).unwrap(),
            CatchUp::Snapshot,
            "ahead of server"
        );
        for i in 0..(KEEP_EVENTS * 2 + 5) {
            run(&mut c, &dm, rename(id, &format!("T{i}")));
        }
        assert_eq!(
            catch_up(&c, id, Seq(2)).unwrap(),
            CatchUp::Snapshot,
            "too far behind"
        );
        compact(&mut c, crate::now()).unwrap();
        let head = load_campaign(&c, id).unwrap().unwrap().0;
        let kept: u64 = c
            .query_row(
                "SELECT COUNT(*) FROM events WHERE campaign_id = ?1",
                [id.as_bytes()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(kept, KEEP_EVENTS);
        assert!(
            matches!(catch_up(&c, id, Seq(head.0 - 10)).unwrap(), CatchUp::Events(e) if e.len() == 10)
        );
        assert_eq!(
            catch_up(&c, id, Seq(head.0 - KEEP_EVENTS - 1)).unwrap(),
            CatchUp::Snapshot
        );
    }
}
