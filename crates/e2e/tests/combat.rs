#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use beholder_client_core::Client;
use beholder_client_core::ClientEvent;
use beholder_client_core::cache::Cache;
use beholder_core::character::HitPoints;
use beholder_core::combat::{Colour, Combatant, EncounterOp, Health, Shape, ShapeKind, Source};
use beholder_core::{CampaignId, CampaignTitle, DisplayName, UnixMillis, Username};
use beholder_core::{
    CampaignOp, CampaignSettings, CombatVisibility, CombatantId, EncounterId, ShapeId,
};
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
async fn encounters_are_filtered_and_tokens_move() {
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
    let (dm, mut dm_events) = Client::start(
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

    let fight = EncounterId(uuid::Uuid::now_v7());
    let enc = |c: &Client, op| {
        c.submit(CommandKind::Encounter {
            campaign_id: campaign,
            op,
        })
        .unwrap();
    };
    let monster = |hidden: bool| Combatant {
        id: CombatantId(uuid::Uuid::now_v7()),
        name: if hidden {
            "Lurker".into()
        } else {
            "Goblin".into()
        },
        source: Source::Custom,
        initiative: Some(12),
        hp: HitPoints {
            current: 4,
            max: 20,
            temp: 0,
        },
        ac: Some(15),
        conditions: Vec::new(),
        concentration: false,
        hidden,
        health: None,
    };
    let bran_id = campaign_of(&dm)
        .unwrap()
        .members
        .iter()
        .find(|m| m.display_name.as_str() == "bran")
        .unwrap()
        .user_id;
    let token = ShapeId(uuid::Uuid::now_v7());
    enc(
        &dm,
        EncounterOp::Create {
            id: fight,
            name: "Bridge".into(),
        },
    );
    enc(
        &dm,
        EncounterOp::PutCombatant {
            id: fight,
            combatant: monster(false),
        },
    );
    enc(
        &dm,
        EncounterOp::PutCombatant {
            id: fight,
            combatant: monster(true),
        },
    );
    enc(
        &dm,
        EncounterOp::PutShape {
            id: fight,
            shape: Shape {
                id: token,
                kind: ShapeKind::Token,
                x: 1,
                y: 1,
                w: 1,
                h: 1,
                label: "B".into(),
                colour: Colour::Player(1),
                hidden: false,
                combatant: None,
                owner: Some(bran_id),
                note: String::new(),
            },
        },
    );
    dm.submit(CommandKind::Campaign {
        campaign_id: campaign,
        op: CampaignOp::UpdateSettings(CampaignSettings {
            dm_can_read_private_notes: false,
            combat_visibility: CombatVisibility::Vague,
        }),
    })
    .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        campaign_of(&bran).unwrap().encounters.is_empty(),
        "prepared encounters stay with the DM"
    );
    enc(
        &dm,
        EncounterOp::Activate {
            id: fight,
            active: true,
        },
    );
    wait_for("the player gets the active encounter, filtered", || {
        campaign_of(&bran).is_some_and(|c| {
            c.encounters.first().is_some_and(|e| {
                e.combatants.len() == 1
                    && e.combatants[0].health == Some(Health::NearDeath)
                    && e.combatants[0].hp.current == 0
            })
        })
    })
    .await;
    enc(
        &bran,
        EncounterOp::MoveShape {
            id: fight,
            shape: token,
            x: 6,
            y: 2,
        },
    );
    wait_for("the DM sees the move", || {
        campaign_of(&dm).is_some_and(|c| {
            c.encounters[0]
                .shapes
                .iter()
                .any(|s| s.id == token && s.x == 6)
        })
    })
    .await;
    enc(
        &dm,
        EncounterOp::Settings {
            id: fight,
            locked: false,
            live_drag: true,
        },
    );
    wait_for("live dragging reaches the player", || {
        campaign_of(&bran).is_some_and(|c| c.encounters.first().is_some_and(|e| e.live_drag))
    })
    .await;
    bran.preview(beholder_protocol::Preview {
        campaign_id: campaign,
        encounter: fight,
        shape: token,
        x: 7,
        y: 2,
    });
    let got = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(e) = dm_events.recv().await {
            if let ClientEvent::Preview(p) = e {
                return Some(p);
            }
        }
        None
    })
    .await
    .ok()
    .flatten();
    assert_eq!(
        got.map(|p| (p.shape, p.x)),
        Some((token, 7)),
        "the drag preview is relayed"
    );
    let (d, b) = (dm.clone(), bran.clone());
    let (as_dm, as_bran) = tokio::task::spawn_blocking(move || {
        (
            d.encounter_as(campaign, fight, bran_id),
            b.encounter_as(campaign, fight, bran_id),
        )
    })
    .await
    .unwrap();
    let previewed = as_dm.unwrap().expect("the running encounter reaches Bran");
    let theirs = campaign_of(&bran).unwrap().encounters[0].clone();
    assert_eq!(
        (previewed.combatants.len(), previewed.shapes.len()),
        (theirs.combatants.len(), theirs.shapes.len()),
        "the DM's preview matches what Bran received"
    );
    assert!(as_bran.is_err(), "players can't preview others");
    for c in [&dm, &ilsa, &bran] {
        c.shutdown();
    }
}
