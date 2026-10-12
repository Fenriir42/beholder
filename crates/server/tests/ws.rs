//! End-to-end server tests over real WebSockets, checked at the send boundary:
//! what a client actually receives on the wire.

// Test-only file: helpers panic on unexpected results by design.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::SocketAddr;
use std::time::Duration;

use beholder_core::{
    CampaignEvent, CampaignId, CampaignOp, CampaignTitle, CommandId, DisplayName, InviteCode, Seq,
    UnixMillis, Username,
};
use beholder_protocol::{
    ClientMsg, Command, CommandKind, Hello, HelloError, PROTOCOL_VERSION, RejectReason, ServerMsg,
    SessionToken, codec,
};
use beholder_server::db::Db;
use beholder_server::{AppState, Server, auth, serve};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};
use uuid::Uuid;

struct Harness {
    addr: SocketAddr,
    state: AppState,
    _dir: tempfile::TempDir,
}

async fn start() -> Harness {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    let dir = tempfile::tempdir().unwrap();
    let state = Server::with_db(Db::open_in_memory().unwrap(), dir.path().to_path_buf());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(serve(listener, state.clone(), std::future::pending()));
    Harness {
        addr,
        state,
        _dir: dir,
    }
}

impl Harness {
    async fn user(&self, name: &str) -> SessionToken {
        let name = name.to_owned();
        self.state
            .db
            .call(move |c| {
                let u = auth::create_user(
                    c,
                    &Username::new(name.clone()).unwrap(),
                    &DisplayName::new(&name).unwrap(),
                    "phc",
                    UnixMillis(0),
                )?
                .unwrap();
                auth::create_session(c, u.id, beholder_server::now())
            })
            .await
            .unwrap()
    }
}

struct Client {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    /// Decompressed postcard bodies of everything received.
    raw: Vec<Vec<u8>>,
}

impl Client {
    async fn open(addr: SocketAddr) -> Self {
        let (ws, _) = connect_async(format!("ws://{addr}/ws")).await.unwrap();
        Self {
            ws,
            raw: Vec::new(),
        }
    }

    async fn connect(
        addr: SocketAddr,
        token: &SessionToken,
        known: Vec<(CampaignId, Seq)>,
    ) -> Self {
        let mut c = Self::open(addr).await;
        c.send(ClientMsg::Hello(Hello {
            protocol_version: PROTOCOL_VERSION,
            session_token: token.clone(),
            known,
        }))
        .await;
        match c.recv().await {
            ServerMsg::HelloOk(_) => c,
            other => panic!("expected HelloOk, got {other:?}"),
        }
    }

    async fn send(&mut self, msg: ClientMsg) {
        let frame = codec::encode(&msg).unwrap();
        self.ws.send(Message::Binary(frame.into())).await.unwrap();
    }

    async fn command(&mut self, kind: CommandKind) -> CommandId {
        let id = CommandId(Uuid::now_v7());
        self.send(ClientMsg::Command(Command { id, kind })).await;
        id
    }

    async fn try_recv(&mut self, wait: Duration) -> Option<ServerMsg> {
        loop {
            let msg = tokio::time::timeout(wait, self.ws.next())
                .await
                .ok()??
                .ok()?;
            if let Message::Binary(frame) = msg {
                let m: ServerMsg = codec::decode(&frame).unwrap();
                self.raw.push(postcard::to_stdvec(&m).unwrap());
                return Some(m);
            }
        }
    }

    async fn recv(&mut self) -> ServerMsg {
        self.try_recv(Duration::from_secs(5))
            .await
            .expect("expected a message")
    }

    /// Receives until `pick` returns Some, skipping presence noise.
    async fn wait_for<T>(&mut self, mut pick: impl FnMut(&ServerMsg) -> Option<T>) -> T {
        loop {
            let m = self.recv().await;
            if let Some(t) = pick(&m) {
                return t;
            }
        }
    }

    async fn ack(&mut self, id: CommandId) -> Option<Seq> {
        self.wait_for(|m| match m {
            ServerMsg::Ack { id: i, seq } if *i == id => Some(*seq),
            ServerMsg::Reject { id: i, reason } if *i == id => panic!("rejected: {reason:?}"),
            _ => None,
        })
        .await
    }

    async fn reject(&mut self, id: CommandId) -> RejectReason {
        self.wait_for(|m| match m {
            ServerMsg::Reject { id: i, reason } if *i == id => Some(*reason),
            ServerMsg::Ack { id: i, .. } if *i == id => panic!("unexpected ack"),
            _ => None,
        })
        .await
    }

    fn saw_bytes(&self, needle: &[u8]) -> bool {
        self.raw
            .iter()
            .any(|b| b.windows(needle.len()).any(|w| w == needle))
    }
}

fn title(s: &str) -> CampaignTitle {
    CampaignTitle::new(s).unwrap()
}

/// DM creates a campaign and returns its id and invite code.
async fn create_campaign(dm: &mut Client) -> (CampaignId, InviteCode) {
    let campaign_id = CampaignId(Uuid::now_v7());
    let id = dm
        .command(CommandKind::CreateCampaign {
            campaign_id,
            title: title("Strahd"),
        })
        .await;
    // The snapshot is queued inside the commit, so it always precedes the Ack.
    let code = dm
        .wait_for(|m| match m {
            ServerMsg::Snapshot { campaign, .. } if campaign.id == campaign_id => {
                campaign.invite_code
            }
            _ => None,
        })
        .await;
    dm.ack(id).await;
    (campaign_id, code)
}

#[tokio::test]
async fn player_never_receives_the_invite_code() {
    let h = start().await;
    let dm_token = h.user("dmaster").await;
    let player_token = h.user("player").await;
    let mut dm = Client::connect(h.addr, &dm_token, vec![]).await;
    let (cid, code) = create_campaign(&mut dm).await;

    let mut player = Client::connect(h.addr, &player_token, vec![]).await;
    let join = player.command(CommandKind::JoinCampaign { code }).await;
    let snapshot_seq = player
        .wait_for(|m| match m {
            ServerMsg::Snapshot { seq, campaign } => {
                assert_eq!(campaign.invite_code, None);
                assert_eq!(campaign.members.len(), 2);
                Some(*seq)
            }
            _ => None,
        })
        .await;
    player.ack(join).await;

    // Live: the DM regenerates, then renames. The player sees only the rename,
    // with a gap in seq where the regeneration was.
    let regen = dm
        .command(CommandKind::Campaign {
            campaign_id: cid,
            op: CampaignOp::RegenerateInvite,
        })
        .await;
    let new_code = dm
        .wait_for(|m| match m {
            ServerMsg::Events { events, .. } => events.iter().find_map(|e| match e.event {
                CampaignEvent::InviteRegenerated { code } => Some(code),
                _ => None,
            }),
            _ => None,
        })
        .await;
    dm.ack(regen).await;
    let rename = dm
        .command(CommandKind::Campaign {
            campaign_id: cid,
            op: CampaignOp::Rename {
                title: title("Barovia"),
            },
        })
        .await;
    let rename_seq = dm.ack(rename).await.unwrap();
    let got = player
        .wait_for(|m| match m {
            ServerMsg::Events { events, .. } => Some(events.clone()),
            _ => None,
        })
        .await;
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].seq, rename_seq);
    assert!(matches!(got[0].event, CampaignEvent::Renamed { .. }));

    // Forbidden for players, at the server.
    let attempt = player
        .command(CommandKind::Campaign {
            campaign_id: cid,
            op: CampaignOp::RegenerateInvite,
        })
        .await;
    assert_eq!(player.reject(attempt).await, RejectReason::Forbidden);

    // Catch-up after reconnecting, both by replay and by snapshot.
    let mut replay = Client::connect(h.addr, &player_token, vec![(cid, snapshot_seq)]).await;
    let mut fresh = Client::connect(h.addr, &player_token, vec![]).await;
    for c in [&mut replay, &mut fresh] {
        while c.try_recv(Duration::from_millis(300)).await.is_some() {}
    }
    for (name, c) in [("live", &player), ("replay", &replay), ("snapshot", &fresh)] {
        for code in [code, new_code] {
            assert!(
                !c.saw_bytes(code.as_str().as_bytes()),
                "{name}: player received an invite code"
            );
        }
    }
    // Sanity: the same scan does find the code in the DM's traffic.
    assert!(dm.saw_bytes(new_code.as_str().as_bytes()));
}

#[tokio::test]
async fn resent_command_is_applied_once() {
    let h = start().await;
    let token = h.user("dmaster").await;
    let mut dm = Client::connect(h.addr, &token, vec![]).await;
    let (cid, _) = create_campaign(&mut dm).await;
    let cmd = Command {
        id: CommandId(Uuid::now_v7()),
        kind: CommandKind::Campaign {
            campaign_id: cid,
            op: CampaignOp::Rename {
                title: title("Once"),
            },
        },
    };
    dm.send(ClientMsg::Command(cmd.clone())).await;
    let first = dm.ack(cmd.id).await;
    // Resent on a new connection, as after a reconnect.
    let mut again = Client::connect(h.addr, &token, vec![(cid, first.unwrap())]).await;
    again.send(ClientMsg::Command(cmd.clone())).await;
    assert_eq!(again.ack(cmd.id).await, first);
    let head = h
        .state
        .db
        .call(move |c| beholder_server::campaigns::load_campaign(c, cid))
        .await
        .unwrap()
        .unwrap()
        .0;
    assert_eq!(Some(head), first, "no second event was appended");
}

#[tokio::test]
async fn stale_and_bad_commands_are_rejected() {
    let h = start().await;
    let token = h.user("dmaster").await;
    let mut dm = Client::connect(h.addr, &token, vec![]).await;
    // A UUIDv7 from 2020 is outside the dedup window.
    let old = CommandId(Uuid::from_bytes([
        0x01, 0x70, 0, 0, 0, 0, 0x70, 0, 0x80, 0, 0, 0, 0, 0, 0, 1,
    ]));
    dm.send(ClientMsg::Command(Command {
        id: old,
        kind: CommandKind::JoinCampaign {
            code: InviteCode::parse("ABCDEFGH").unwrap(),
        },
    }))
    .await;
    assert_eq!(dm.reject(old).await, RejectReason::Expired);
    let bad = dm
        .command(CommandKind::JoinCampaign {
            code: InviteCode::parse("ZZZZZZZZ").unwrap(),
        })
        .await;
    assert_eq!(dm.reject(bad).await, RejectReason::InvalidInviteCode);
    let missing = dm
        .command(CommandKind::Campaign {
            campaign_id: CampaignId(Uuid::now_v7()),
            op: CampaignOp::RegenerateInvite,
        })
        .await;
    assert_eq!(dm.reject(missing).await, RejectReason::NotFound);
}

#[tokio::test]
async fn handshake_refuses_old_clients_and_bad_tokens() {
    let h = start().await;
    let token = h.user("dmaster").await;
    let mut c = Client::open(h.addr).await;
    c.send(ClientMsg::Hello(Hello {
        protocol_version: PROTOCOL_VERSION + 1,
        session_token: token,
        known: vec![],
    }))
    .await;
    assert_eq!(
        c.recv().await,
        ServerMsg::HelloErr(HelloError::UpdateRequired {
            server_version: PROTOCOL_VERSION
        })
    );
    let mut c = Client::open(h.addr).await;
    c.send(ClientMsg::Hello(Hello {
        protocol_version: PROTOCOL_VERSION,
        session_token: SessionToken([0; 32]),
        known: vec![],
    }))
    .await;
    assert_eq!(
        c.recv().await,
        ServerMsg::HelloErr(HelloError::InvalidSession)
    );
}

#[tokio::test]
async fn ping_is_answered_and_presence_is_shared() {
    let h = start().await;
    let dm_token = h.user("dmaster").await;
    let p_token = h.user("player").await;
    let mut dm = Client::connect(h.addr, &dm_token, vec![]).await;
    let (cid, code) = create_campaign(&mut dm).await;
    dm.send(ClientMsg::Ping { t: 42 }).await;
    dm.wait_for(|m| matches!(m, ServerMsg::Pong { t: 42 }).then_some(()))
        .await;

    let mut player = Client::connect(h.addr, &p_token, vec![]).await;
    let join = player.command(CommandKind::JoinCampaign { code }).await;
    let player_id = player
        .wait_for(|m| match m {
            ServerMsg::Snapshot { campaign, .. } => Some(campaign.members[1].user_id),
            _ => None,
        })
        .await;
    player.ack(join).await;
    dm.wait_for(|m| match m {
        ServerMsg::PresenceChanged {
            campaign_id,
            user_id,
            online: true,
        } if *campaign_id == cid && *user_id == player_id => Some(()),
        _ => None,
    })
    .await;
}
