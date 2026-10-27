#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use beholder_client_core::cache::Cache;
use beholder_client_core::{Client, ClientEvent, CompendiumState};
use beholder_core::CampaignOp;
use beholder_core::compendium::{Kind, SearchQuery};
use beholder_core::{CampaignId, CampaignTitle, DisplayName, UnixMillis, Username};
use beholder_protocol::CommandKind;
use beholder_server::compendium::Library;
use beholder_server::db::Db;
use beholder_server::{Server, auth, serve};
use tokio::net::TcpListener;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn client_downloads_indexes_and_searches_the_srd() {
    let dir = tempfile::tempdir().unwrap();
    let state = Server::with_db(
        Db::open_in_memory().unwrap(),
        dir.path().to_path_buf(),
        Library::builtin().unwrap(),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(serve(listener, state.clone(), std::future::pending()));
    let token = state
        .db
        .call(|c| {
            let u = auth::create_user(
                c,
                &Username::new("reader".into()).unwrap(),
                &DisplayName::new("Reader").unwrap(),
                "phc",
                UnixMillis(0),
            )?
            .unwrap();
            auth::create_session(c, u.id, beholder_server::now())
        })
        .await
        .unwrap();
    let (client, mut events) = Client::start(&url, Cache::open_in_memory().unwrap(), token);
    let campaign_id = CampaignId(uuid::Uuid::now_v7());
    client
        .submit(CommandKind::CreateCampaign {
            campaign_id,
            title: CampaignTitle::new("Lookup").unwrap(),
        })
        .unwrap();
    let ready = tokio::time::timeout(Duration::from_secs(30), async {
        while let Some(e) = events.recv().await {
            match e {
                ClientEvent::Compendium(CompendiumState::Ready)
                    if !client.compendium_sources().unwrap().is_empty() =>
                {
                    return true;
                }
                ClientEvent::Compendium(CompendiumState::Failed) => return false,
                _ => {}
            }
        }
        false
    })
    .await
    .unwrap();
    assert!(ready, "compendium sync failed");

    let sources = client.compendium_sources().unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].id, "srd-5.2");
    let hits = client
        .search(
            &SearchQuery {
                text: "owlbear".into(),
                kind: Some(Kind::Monster),
                sources: vec!["srd-5.2".into()],
                ..SearchQuery::default()
            },
            10,
        )
        .unwrap();
    assert_eq!(hits.first().map(|h| h.name.as_str()), Some("Owlbear"));
    let (entry, _) = client
        .compendium_entry("srd-5.2", &hits[0].key)
        .unwrap()
        .unwrap();
    assert!(entry.text.contains("**AC**"));

    let both = vec!["srd-5.2".to_owned(), "srd-2014".to_owned()];
    client
        .submit(CommandKind::Campaign {
            campaign_id,
            op: CampaignOp::SetSources {
                sources: both.clone(),
            },
        })
        .unwrap();
    let synced = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if client.compendium_sources().unwrap().len() == 2 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(synced.is_ok(), "enabling a source downloads it");
    let hits = client
        .search(
            &SearchQuery {
                text: "fireball".into(),
                kind: Some(Kind::Spell),
                sources: both,
                ..SearchQuery::default()
            },
            10,
        )
        .unwrap();
    let fireball = hits.iter().find(|h| h.name == "Fireball").unwrap();
    assert_eq!(
        fireball.source, "srd-5.2",
        "the 2024 SRD wins the name clash"
    );
    assert_eq!(fireball.others, 1);
    client.shutdown();
}
