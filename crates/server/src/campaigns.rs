//! Campaign commands, the event log and its SQL projection.
//!
//! Every function here runs inside [`crate::db::Db::call`], so calls are
//! serialized and each command is one transaction: dedupe check, validation,
//! event append, projection update and stored reply commit together.

use beholder_core::{
    Campaign, CampaignEvent, CampaignId, CampaignOp, CampaignSettings, CampaignTitle,
    CombatVisibility, CommandId, DisplayName, ImageHash, InviteCode, Member, Role, Seq, UnixMillis,
    User, UserId, ValidationError,
};
use beholder_protocol::{Command, CommandKind, EventEnvelope, RejectReason};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::db::DbError;
use crate::permissions;

/// Commands older than this are refused: the server may have forgotten them,
/// so applying them again could duplicate their effect.
pub const DEDUP_WINDOW_MS: i64 = 7 * 24 * 3600 * 1000;

/// Stored replies are kept a day longer than the window, so there is no gap.
pub const COMMAND_RETENTION_MS: i64 = DEDUP_WINDOW_MS + 24 * 3600 * 1000;

/// A client further behind than this gets a snapshot instead of replayed events.
pub const MAX_REPLAY: u64 = 1000;

/// Events kept per campaign by compaction; older ones are covered by snapshots.
pub const KEEP_EVENTS: u64 = 1000;

/// The answer to a command, stored for deduplication.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reply {
    /// Applied; seq of the last event produced, if any.
    Ack(Option<Seq>),
    /// Refused.
    Reject(RejectReason),
}

/// The result of handling one command.
#[derive(Debug, PartialEq, Eq)]
pub struct Handled {
    /// What to answer the sender.
    pub reply: Reply,
    /// The campaign the new events belong to.
    pub campaign_id: Option<CampaignId>,
    /// New events to fan out (unfiltered; the hub filters per recipient).
    pub events: Vec<EventEnvelope>,
    /// Set when the sender just became a member (create or join) with this role.
    pub joined: Option<Role>,
}

impl Handled {
    fn reply_only(reply: Reply) -> Self {
        Self {
            reply,
            campaign_id: None,
            events: Vec::new(),
            joined: None,
        }
    }
}

struct Executed {
    campaign_id: CampaignId,
    events: Vec<EventEnvelope>,
    joined: Option<Role>,
}

/// Handles one command from `actor`, at most once per command id.
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
            reply: Reply::Ack(ex.events.last().map(|e| e.seq)),
            campaign_id: Some(ex.campaign_id),
            events: ex.events,
            joined: ex.joined,
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
                campaign_id: *campaign_id,
                events: vec![created, code],
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
                // Already a member (e.g. the DM typed their own code): nothing to record.
                return Ok(Ok(Executed {
                    campaign_id,
                    events: Vec::new(),
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
                campaign_id,
                events: vec![ev],
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
                campaign_id: *campaign_id,
                events: vec![ev],
                joined: None,
            }))
        }
    }
}

/// Appends events and keeps the projection in step, inside one transaction.
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

/// Applies one event to the SQL projection; mirrors `Campaign::apply`.
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
    }
    Ok(())
}

/// The current state of a campaign and the seq it is current at.
pub fn load_campaign(
    conn: &Connection,
    id: CampaignId,
) -> Result<Option<(Seq, Campaign)>, DbError> {
    let row = conn
        .query_row(
            "SELECT title, cover_hash, invite_code, dm_can_read_private_notes, combat_visibility, head_seq
             FROM campaigns WHERE id = ?1",
            [id.as_bytes()],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<Vec<u8>>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, bool>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, u64>(5)?,
                ))
            },
        )
        .optional()?;
    let Some((title, cover, code, dm_read, vis, head)) = row else {
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
    };
    Ok(Some((Seq(head), campaign)))
}

/// What a client that has seen up to `after` needs.
#[derive(Debug, PartialEq, Eq)]
pub enum CatchUp {
    /// These events (possibly none).
    Events(Vec<EventEnvelope>),
    /// Too far behind, or ahead of the server: send a snapshot.
    Snapshot,
}

/// Events after `after`, or `Snapshot` when replay isn't possible or worthwhile.
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
        .collect::<Result<_, DbError>>()?;
    Ok(CatchUp::Events(events))
}

/// Every campaign `user` belongs to, with their role.
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

/// Drops old events (keeping [`KEEP_EVENTS`] per campaign) and expired command replies.
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

fn role_of(
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

/// Reads a 16-byte id column.
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
    use beholder_core::Username;
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

        let (head, projected) = load_campaign(&c, id).unwrap().unwrap();
        let CatchUp::Events(events) = catch_up(&c, id, Seq(1)).unwrap() else {
            panic!("expected events");
        };
        let CampaignEvent::Created { title, dm: first } = &catch_up_all(&c, id)[0].event else {
            panic!("first event must be Created");
        };
        let mut folded = Campaign::new(id, title.clone(), first.clone());
        for e in &events {
            folded.apply(&e.event);
        }
        assert_eq!(head, Seq(6));
        assert_eq!(projected, folded);
        assert_ne!(projected.invite_code, Some(code));
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
        // Another user reusing the id is refused and changes nothing.
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
