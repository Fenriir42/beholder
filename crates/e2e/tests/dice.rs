#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use beholder_client_core::Client;
use beholder_client_core::cache::Cache;
use beholder_core::dice::{DiceOp, Expr, Visibility};
use beholder_core::{CampaignId, CampaignTitle, DisplayName, UnixMillis, Username};
use beholder_protocol::CommandKind;
use beholder_server::compendium::Library;
use beholder_server::db::Db;
use beholder_server::{Server, auth, serve};
use tokio::net::TcpListener;

async fn wait_for(what: &str, mut check: impl FnMut() -> bool) {
    let ok = tokio::time::timeout(Duration::from_secs(30), async {
        while !check() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(ok.is_ok(), "timed out: {what}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rolls_reach_who_may_see_them() {
    let dir = tempfile::tempdir().unwrap();
    let state = Server::with_db(
        Db::open_in_memory().unwrap(),
        dir.path().to_path_buf(),
        Library::builtin().unwrap(),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(serve(listener, state.clone(), std::future::pending()));
    let token = |name: &'static str| {
        let state = state.clone();
        async move {
            state
                .db
                .call(move |c| {
                    let u = auth::create_user(
                        c,
                        &Username::new(name.into()).unwrap(),
                        &DisplayName::new(name).unwrap(),
                        "phc",
                        UnixMillis(0),
                    )?
                    .unwrap();
                    auth::create_session(c, u.id, beholder_server::now())
                })
                .await
                .unwrap()
        }
    };
    let (dm, _e1) = Client::start(
        &url,
        Cache::open_in_memory().unwrap(),
        token("dmaster").await,
    );
    let (ilsa, _e2) = Client::start(&url, Cache::open_in_memory().unwrap(), token("ilsa").await);
    let (bran, _e3) = Client::start(&url, Cache::open_in_memory().unwrap(), token("bran").await);

    let campaign = CampaignId(uuid::Uuid::now_v7());
    dm.submit(CommandKind::CreateCampaign {
        campaign_id: campaign,
        title: CampaignTitle::new("Sheets").unwrap(),
    })
    .unwrap();
    let campaign_of = |c: &Client| {
        c.view()
            .unwrap()
            .campaigns
            .into_iter()
            .find(|v| v.campaign.id == campaign && !v.pending)
            .map(|v| v.campaign)
    };
    wait_for("the DM gets an invite code", || {
        campaign_of(&dm).is_some_and(|c| c.invite_code.is_some())
    })
    .await;
    let code = campaign_of(&dm).unwrap().invite_code.unwrap();
    for p in [&ilsa, &bran] {
        p.submit(CommandKind::JoinCampaign { code }).unwrap();
    }
    wait_for("both players joined", || {
        campaign_of(&ilsa).is_some() && campaign_of(&bran).is_some()
    })
    .await;

    let roll = |c: &Client, context: &str, visibility| {
        c.submit(CommandKind::Dice {
            campaign_id: campaign,
            op: DiceOp::Roll {
                expr: Expr::parse("1d20+3").unwrap(),
                context: context.into(),
                character: None,
                visibility,
                targets: vec![beholder_core::CombatantId(uuid::Uuid::from_bytes([9; 16]))],
            },
        })
        .unwrap()
    };
    let public = roll(&ilsa, "Stealth", Visibility::Public);
    let secret = roll(&ilsa, "Insight", Visibility::ToDm);
    let has = |c: &Client, id| campaign_of(c).is_some_and(|k| k.rolls.iter().any(|r| r.id == id));
    wait_for("the DM sees both rolls", || {
        has(&dm, public) && has(&dm, secret)
    })
    .await;
    wait_for("the roller sees both", || {
        has(&ilsa, public) && has(&ilsa, secret)
    })
    .await;
    wait_for("the other player sees the public roll", || {
        has(&bran, public)
    })
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !has(&bran, secret),
        "a roll to the DM never reaches other players"
    );
    let total = campaign_of(&ilsa)
        .unwrap()
        .rolls
        .iter()
        .find(|r| r.id == public)
        .map(|r| r.outcome.total)
        .unwrap();
    assert!((4..=23).contains(&total));
    assert_eq!(
        campaign_of(&bran)
            .unwrap()
            .rolls
            .iter()
            .find(|r| r.id == public)
            .map(|r| r.targets.len()),
        Some(1),
        "targets travel with the roll"
    );
    for c in [&dm, &ilsa, &bran] {
        c.shutdown();
    }
}
