#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use beholder_client_core::Client;
use beholder_client_core::cache::Cache;
use beholder_core::character::{CharacterChange, CharacterOp, HitPoints};
use beholder_core::compendium::RulesVersion;
use beholder_core::{CampaignId, CampaignTitle, CharacterId, DisplayName, UnixMillis, Username};
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
async fn sheets_sync_to_owner_and_dm_and_summaries_to_the_party() {
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

    let id = CharacterId(uuid::Uuid::now_v7());
    let cmd = |op| CommandKind::Character {
        campaign_id: campaign,
        op,
    };
    ilsa.submit(cmd(CharacterOp::Create {
        id,
        name: "Ilsa the Bold".into(),
        rules: RulesVersion::R2024,
    }))
    .unwrap();
    ilsa.submit(cmd(CharacterOp::Change {
        id,
        change: CharacterChange::Notes("owes the guild".into()),
    }))
    .unwrap();
    wait_for("the DM sees the full sheet", || {
        campaign_of(&dm)
            .and_then(|c| c.character(id).cloned())
            .is_some_and(|ch| ch.notes == "owes the guild")
    })
    .await;
    dm.submit(cmd(CharacterOp::Change {
        id,
        change: CharacterChange::Hp(HitPoints {
            current: 4,
            max: 12,
            temp: 0,
        }),
    }))
    .unwrap();
    wait_for("the owner gets the DM's change", || {
        campaign_of(&ilsa)
            .and_then(|c| c.character(id).cloned())
            .is_some_and(|ch| ch.hp.current == 4 && ch.notes == "owes the guild")
    })
    .await;
    wait_for("the other player sees only a summary", || {
        campaign_of(&bran).is_some_and(|c| {
            c.characters.is_empty()
                && c.party
                    .iter()
                    .any(|s| s.id == id && s.hp.current == 4 && s.name == "Ilsa the Bold")
        })
    })
    .await;
    bran.submit(cmd(CharacterOp::Change {
        id,
        change: CharacterChange::Notes("hijacked".into()),
    }))
    .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(
        campaign_of(&dm).unwrap().character(id).unwrap().notes,
        "owes the guild",
        "another player can't edit the sheet"
    );
    for c in [&dm, &ilsa, &bran] {
        c.shutdown();
    }
}
