#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::SocketAddr;
use std::time::Duration;

use beholder_core::{
    Audience, CampaignEvent, CampaignId, CampaignOp, CampaignSettings, CampaignTitle, CommandId,
    DisplayName, InviteCode, NoteBody, NoteId, NoteOp, NoteTitle, Seq, UnixMillis, UserId,
    Username,
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
    let state = Server::with_db(
        Db::open_in_memory().unwrap(),
        dir.path().to_path_buf(),
        beholder_server::compendium::Library::builtin().unwrap(),
    );
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

    async fn drain(&mut self) {
        while self.try_recv(Duration::from_millis(300)).await.is_some() {}
    }

    fn saw(&self, text: &str) -> bool {
        self.saw_bytes(text.as_bytes())
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

async fn create_campaign(dm: &mut Client) -> (CampaignId, InviteCode) {
    let campaign_id = CampaignId(Uuid::now_v7());
    let id = dm
        .command(CommandKind::CreateCampaign {
            campaign_id,
            title: title("Strahd"),
        })
        .await;
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

    let attempt = player
        .command(CommandKind::Campaign {
            campaign_id: cid,
            op: CampaignOp::RegenerateInvite,
        })
        .await;
    assert_eq!(player.reject(attempt).await, RejectReason::Forbidden);

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
    let (cid, _) = create_campaign(&mut dm).await;
    let unknown = dm
        .command(CommandKind::Campaign {
            campaign_id: cid,
            op: CampaignOp::SetSources {
                sources: vec!["srd-5.2".into(), "not-a-source".into()],
            },
        })
        .await;
    assert_eq!(dm.reject(unknown).await, RejectReason::NotFound);
    let known = dm
        .command(CommandKind::Campaign {
            campaign_id: cid,
            op: CampaignOp::SetSources { sources: vec![] },
        })
        .await;
    assert!(
        dm.ack(known).await.is_some(),
        "a DM may turn every source off"
    );
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

async fn join(player: &mut Client, code: InviteCode) -> UserId {
    let id = player.command(CommandKind::JoinCampaign { code }).await;
    player
        .wait_for(|m| matches!(m, ServerMsg::Snapshot { .. }).then_some(()))
        .await;
    player.ack(id).await;
    let token_user = player
        .raw
        .iter()
        .find_map(|b| match postcard::from_bytes::<ServerMsg>(b) {
            Ok(ServerMsg::Snapshot { campaign, .. }) => campaign.members.last().map(|m| m.user_id),
            _ => None,
        });
    token_user.unwrap()
}

async fn note(c: &mut Client, campaign_id: CampaignId, op: NoteOp) -> Option<Seq> {
    let id = c.command(CommandKind::Note { campaign_id, op }).await;
    c.ack(id).await
}

fn create(id: NoteId, body: &str, audience: Audience) -> NoteOp {
    NoteOp::Create {
        id,
        title: NoteTitle::new("Note").unwrap(),
        body: NoteBody::new(body.into()).unwrap(),
        audience,
    }
}

#[tokio::test]
async fn notes_reach_only_their_audience() {
    let h = start().await;
    let (dm_t, p1_t, p2_t) = (
        h.user("dmaster").await,
        h.user("alice").await,
        h.user("bob").await,
    );
    let mut dm = Client::connect(h.addr, &dm_t, vec![]).await;
    let (cid, code) = create_campaign(&mut dm).await;
    let mut p1 = Client::connect(h.addr, &p1_t, vec![]).await;
    let alice = join(&mut p1, code).await;
    let mut p2 = Client::connect(h.addr, &p2_t, vec![]).await;
    join(&mut p2, code).await;

    let dm_only = NoteId(Uuid::now_v7());
    note(
        &mut dm,
        cid,
        create(dm_only, "SECRET-DM-NOTE", Audience::Private),
    )
    .await;
    note(
        &mut p1,
        cid,
        create(NoteId(Uuid::now_v7()), "SECRET-ALICE", Audience::Private),
    )
    .await;
    let for_alice = create(
        NoteId(Uuid::now_v7()),
        "FOR-ALICE",
        Audience::Users(vec![alice]),
    );
    note(&mut dm, cid, for_alice).await;
    let edit = NoteOp::Edit {
        id: dm_only,
        base_version: 1,
        title: NoteTitle::new("Note").unwrap(),
        body: NoteBody::new("SECRET-DM-EDIT".into()).unwrap(),
    };
    let last = note(&mut dm, cid, edit).await.unwrap();
    for c in [&mut dm, &mut p1, &mut p2] {
        c.drain().await;
    }

    let mut replay = Client::connect(h.addr, &p2_t, vec![(cid, Seq(last.0 - 3))]).await;
    let mut fresh = Client::connect(h.addr, &p2_t, vec![]).await;
    for c in [&mut replay, &mut fresh] {
        c.drain().await;
    }
    for secret in [
        "SECRET-DM-NOTE",
        "SECRET-DM-EDIT",
        "SECRET-ALICE",
        "FOR-ALICE",
    ] {
        for (name, c) in [("live", &p2), ("replay", &replay), ("snapshot", &fresh)] {
            assert!(!c.saw(secret), "bob ({name}) received {secret}");
        }
    }
    assert!(!p1.saw("SECRET-DM-NOTE") && !p1.saw("SECRET-DM-EDIT"));
    assert!(p1.saw("FOR-ALICE"));
    assert!(
        !dm.saw("SECRET-ALICE"),
        "DM reads private notes only when enabled"
    );

    let enable = dm
        .command(CommandKind::Campaign {
            campaign_id: cid,
            op: CampaignOp::UpdateSettings(CampaignSettings {
                dm_can_read_private_notes: true,
                ..CampaignSettings::default()
            }),
        })
        .await;
    dm.ack(enable).await;
    dm.drain().await;
    assert!(
        dm.saw("SECRET-ALICE"),
        "enabling the setting sends the DM a snapshot"
    );

    let reveal = |audience| NoteOp::SetAudience {
        id: dm_only,
        audience,
    };
    note(&mut dm, cid, reveal(Audience::Party)).await;
    let revealed = p2
        .wait_for(|m| match m {
            ServerMsg::Events { events, .. } => events.iter().find_map(|e| match &e.event {
                CampaignEvent::NoteCreated { note } if note.id == dm_only => Some(note.clone()),
                _ => None,
            }),
            _ => None,
        })
        .await;
    assert_eq!(
        revealed.body.as_str(),
        "SECRET-DM-EDIT",
        "a reveal sends the current text"
    );

    note(&mut dm, cid, reveal(Audience::Private)).await;
    p2.wait_for(|m| match m {
        ServerMsg::Events { events, .. } => events
            .iter()
            .any(|e| e.event == CampaignEvent::NoteDeleted { id: dm_only })
            .then_some(()),
        _ => None,
    })
    .await;
    assert!(!p2.saw("SECRET-ALICE"));
}

async fn http_get(addr: SocketAddr, path: &str, headers: &[(&str, String)]) -> (u16, Vec<u8>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut s = TcpStream::connect(addr).await.unwrap();
    let mut req = format!("GET {path} HTTP/1.1\r\nHost: test\r\nConnection: close\r\n");
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    s.write_all(req.as_bytes()).await.unwrap();
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).await.unwrap();
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&raw[..split]).to_string();
    let status = head[9..12].parse().unwrap();
    (status, raw[split + 4..].to_vec())
}

#[tokio::test]
async fn compendium_bundles_download_with_auth_and_resume() {
    use beholder_protocol::http::SourceInfo;
    use sha2::{Digest, Sha256};
    let h = start().await;
    let token = h.user("reader").await;
    let auth = vec![
        ("Authorization", format!("Bearer {}", token.to_hex())),
        ("x-beholder-protocol", PROTOCOL_VERSION.to_string()),
    ];
    let (status, _) = http_get(h.addr, "/api/compendium", &auth[1..]).await;
    assert_eq!(status, 401, "the index needs a session");

    let (status, body) = http_get(h.addr, "/api/compendium", &auth).await;
    assert_eq!(status, 200);
    let index: Vec<SourceInfo> = postcard::from_bytes(&body).unwrap();
    let srd = index.iter().find(|s| s.id == "srd-5.2").unwrap();

    let path = format!("/api/compendium/{}", srd.sha256);
    let (status, full) = http_get(h.addr, &path, &auth).await;
    assert_eq!(status, 200);
    assert_eq!(full.len() as u64, srd.bytes);
    let digest: String = Sha256::digest(&full)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(digest, srd.sha256);

    let mut resume = auth.clone();
    resume.push(("Range", "bytes=1000-".into()));
    let (status, tail) = http_get(h.addr, &path, &resume).await;
    assert_eq!(status, 206);
    assert_eq!(tail, full[1000..]);
    resume.pop();
    resume.push(("Range", format!("bytes={}-", full.len())));
    assert_eq!(http_get(h.addr, &path, &resume).await.0, 416);
    assert_eq!(
        http_get(
            h.addr,
            &format!("/api/compendium/{}", "0".repeat(64)),
            &auth
        )
        .await
        .0,
        404
    );
}
