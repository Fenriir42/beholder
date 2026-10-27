#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use beholder_client_core::cache::Cache;
use beholder_client_core::view::View;
use beholder_client_core::{Client, ClientEvent};
use beholder_core::{
    CampaignId, CampaignOp, CampaignSettings, CampaignTitle, CombatVisibility, CommandId,
    DisplayName, Role, UnixMillis, Username,
};
use beholder_protocol::{CommandKind, RejectReason, SessionToken};
use beholder_server::db::Db;
use beholder_server::{AppState, Server, auth, campaigns, permissions, serve};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{Instant, sleep};

const SEED: u64 = 0xBE40_1DE5;
const DM_COMMANDS: usize = 60;
const PLAYER_COMMANDS: usize = 15;

async fn chaos_proxy(upstream: SocketAddr, accepted: Arc<AtomicUsize>) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let mut rng = fastrand::Rng::with_seed(SEED);
        while let Ok((down, _)) = listener.accept().await {
            accepted.fetch_add(1, Ordering::Relaxed);
            if rng.u8(..8) == 0 {
                drop(down);
                continue;
            }
            let life = Duration::from_millis(rng.u64(200..1500));
            let seed = rng.u64(..);
            tokio::spawn(pipe(down, upstream, life, seed));
        }
    });
    addr
}

async fn pipe(down: TcpStream, upstream: SocketAddr, life: Duration, seed: u64) {
    let Ok(up) = TcpStream::connect(upstream).await else {
        return;
    };
    let (dr, dw) = down.into_split();
    let (ur, uw) = up.into_split();
    tokio::select! {
        () = relay(dr, uw, fastrand::Rng::with_seed(seed)) => {}
        () = relay(ur, dw, fastrand::Rng::with_seed(seed ^ 1)) => {}
        () = sleep(life) => {}
    }
}

async fn relay(
    mut r: tokio::net::tcp::OwnedReadHalf,
    mut w: tokio::net::tcp::OwnedWriteHalf,
    mut rng: fastrand::Rng,
) {
    let mut buf = vec![0u8; 4096];
    loop {
        let n = match r.read(&mut buf).await {
            Ok(0) | Err(_) => return,
            Ok(n) => n,
        };
        sleep(Duration::from_millis(rng.u64(0..40))).await;
        if w.write_all(&buf[..n]).await.is_err() {
            return;
        }
    }
}

async fn user(state: &AppState, name: &str) -> SessionToken {
    let name = name.to_owned();
    state
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

async fn wait_until(
    client: &Client,
    events: &mut UnboundedReceiver<ClientEvent>,
    rejected: &mut Vec<(CommandId, RejectReason)>,
    what: &str,
    done: impl Fn(&View) -> bool,
) -> View {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        while let Ok(e) = events.try_recv() {
            match e {
                ClientEvent::Rejected { id, reason } => rejected.push((id, reason)),
                ClientEvent::SessionInvalid | ClientEvent::UpdateRequired { .. } => {
                    panic!("{what}: fatal {e:?}")
                }
                _ => {}
            }
        }
        let view = client.view().unwrap();
        if done(&view) {
            return view;
        }
        assert!(
            Instant::now() < deadline,
            "{what}: timed out, view = {view:?}"
        );
        sleep(Duration::from_millis(50)).await;
    }
}

fn settled(v: &View, campaign_id: CampaignId) -> bool {
    v.pending_joins == 0
        && v.campaigns.iter().all(|c| !c.pending)
        && v.campaigns
            .iter()
            .any(|c| c.campaign.id == campaign_id && c.seq.0 > 0)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_lost_or_duplicated_commands_and_clients_converge() {
    let dir = tempfile::tempdir().unwrap();
    let state = Server::with_db(Db::open_in_memory().unwrap(), dir.path().to_path_buf());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap();
    tokio::spawn(serve(listener, state.clone(), std::future::pending()));
    let accepted = Arc::new(AtomicUsize::new(0));
    let proxy = format!(
        "http://{}",
        chaos_proxy(server_addr, Arc::clone(&accepted)).await
    );

    let (dm, mut dm_events) = Client::start(
        &proxy,
        Cache::open_in_memory().unwrap(),
        user(&state, "dmaster").await,
    );
    let (player, mut player_events) = Client::start(
        &proxy,
        Cache::open_in_memory().unwrap(),
        user(&state, "player").await,
    );
    let mut dm_rejects = Vec::new();
    let mut player_rejects = Vec::new();

    let campaign_id = CampaignId(uuid::Uuid::now_v7());
    dm.submit(CommandKind::CreateCampaign {
        campaign_id,
        title: CampaignTitle::new("Start").unwrap(),
    })
    .unwrap();
    let v = wait_until(&dm, &mut dm_events, &mut dm_rejects, "create", |v| {
        settled(v, campaign_id)
    })
    .await;
    let code = v.campaigns[0]
        .campaign
        .invite_code
        .expect("DM sees the code");

    player.submit(CommandKind::JoinCampaign { code }).unwrap();
    wait_until(
        &player,
        &mut player_events,
        &mut player_rejects,
        "join",
        |v| settled(v, campaign_id),
    )
    .await;

    let mut dm_ids = Vec::new();
    let mut rng = fastrand::Rng::with_seed(SEED ^ 2);
    for i in 0..DM_COMMANDS {
        let op = match i % 3 {
            0 | 1 => CampaignOp::Rename {
                title: CampaignTitle::new(&format!("T{i}")).unwrap(),
            },
            _ => CampaignOp::UpdateSettings(CampaignSettings {
                dm_can_read_private_notes: rng.bool(),
                combat_visibility: CombatVisibility::Vague,
            }),
        };
        dm_ids.push(
            dm.submit(CommandKind::Campaign { campaign_id, op })
                .unwrap(),
        );
        if i == DM_COMMANDS / 2 {
            dm_ids.push(
                dm.submit(CommandKind::Campaign {
                    campaign_id,
                    op: CampaignOp::RegenerateInvite,
                })
                .unwrap(),
            );
        }
        sleep(Duration::from_millis(rng.u64(20..80))).await;
    }
    let mut player_ids = HashSet::new();
    for _ in 0..PLAYER_COMMANDS {
        player_ids.insert(
            player
                .submit(CommandKind::Campaign {
                    campaign_id,
                    op: CampaignOp::RegenerateInvite,
                })
                .unwrap(),
        );
    }
    let final_title = (0..DM_COMMANDS)
        .rev()
        .find(|i| i % 3 != 2)
        .map(|i| format!("T{i}"))
        .unwrap();

    wait_until(&dm, &mut dm_events, &mut dm_rejects, "dm burst", |v| {
        settled(v, campaign_id)
    })
    .await;
    wait_until(
        &player,
        &mut player_events,
        &mut player_rejects,
        "player burst",
        |v| settled(v, campaign_id),
    )
    .await;

    assert!(dm_rejects.is_empty(), "DM rejects: {dm_rejects:?}");
    let rejected: HashSet<CommandId> = player_rejects.iter().map(|(id, _)| *id).collect();
    assert_eq!(rejected, player_ids);
    assert!(
        player_rejects
            .iter()
            .all(|(_, r)| *r == RejectReason::Forbidden)
    );

    let (counts, head) = state
        .db
        .call(move |c| {
            let mut stmt = c.prepare(
                "SELECT command_id, COUNT(*) FROM events WHERE campaign_id = ?1 GROUP BY command_id",
            )?;
            let counts: Vec<(Vec<u8>, i64)> = stmt
                .query_map([campaign_id.as_bytes()], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<_, _>>()?;
            let head = campaigns::load_campaign(c, campaign_id)?.unwrap();
            Ok::<_, beholder_server::db::DbError>((counts, head))
        })
        .await
        .unwrap();
    assert!(counts.iter().all(|(_, n)| *n <= 2), "{counts:?}");
    for id in &dm_ids {
        let n = counts
            .iter()
            .find(|(c, _)| c.as_slice() == id.as_bytes())
            .map(|(_, n)| *n);
        assert_eq!(n, Some(1), "DM command {id} applied {n:?} times");
    }
    for id in &player_ids {
        assert!(counts.iter().all(|(c, _)| c.as_slice() != id.as_bytes()));
    }
    let (head_seq, server_campaign) = head;
    assert_eq!(server_campaign.title.as_str(), final_title);

    let viewer = |role| {
        let m = server_campaign.members.iter().find(|m| m.role == role);
        permissions::Viewer {
            user: m.map(|m| m.user_id).expect("member with role"),
            role,
        }
    };
    let dm_expected = permissions::campaign_for(viewer(Role::Dm), &server_campaign);
    wait_until(&dm, &mut dm_events, &mut dm_rejects, "dm converge", |v| {
        v.campaigns.len() == 1
            && v.campaigns[0].seq == head_seq
            && v.campaigns[0].campaign == dm_expected
    })
    .await;
    let player_expected = permissions::campaign_for(viewer(Role::Player), &server_campaign);
    assert_eq!(player_expected.invite_code, None);
    wait_until(
        &player,
        &mut player_events,
        &mut player_rejects,
        "player converge",
        |v| v.campaigns.len() == 1 && v.campaigns[0].campaign == player_expected,
    )
    .await;

    let connections = accepted.load(Ordering::Relaxed);
    assert!(
        connections >= 6,
        "only {connections} connections; chaos too mild"
    );

    dm.shutdown();
    player.shutdown();
}
