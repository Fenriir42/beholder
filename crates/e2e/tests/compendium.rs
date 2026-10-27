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
    let ids: Vec<&str> = sources.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        ["library", "srd-5.2"],
        "new campaigns use the SRD and the library"
    );
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
            if client
                .compendium_sources()
                .unwrap()
                .iter()
                .any(|s| s.id == "srd-2014")
            {
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
async fn library_entries_sync_fold_hide_and_import() {
    use beholder_client_core::library_file::FileFormat;
    use beholder_core::compendium::{Facets, RulesVersion};
    use beholder_core::library::LibraryDraft;
    use beholder_core::{LibraryId, NoteBody, NoteTitle};

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
    let (a, _ea) = Client::start(&url, Cache::open_in_memory().unwrap(), token("alice").await);
    let (b, _eb) = Client::start(&url, Cache::open_in_memory().unwrap(), token("bob").await);
    for c in [&a, &b] {
        c.submit(CommandKind::CreateCampaign {
            campaign_id: CampaignId(uuid::Uuid::now_v7()),
            title: CampaignTitle::new("Own").unwrap(),
        })
        .unwrap();
    }
    let sources = vec!["srd-5.2".to_owned(), "library".to_owned()];
    let search = |c: &Client, text: &str, hidden: Vec<String>| {
        c.search(
            &SearchQuery {
                text: text.into(),
                sources: sources.clone(),
                hidden,
                ..SearchQuery::default()
            },
            20,
        )
        .unwrap()
    };

    let id = LibraryId(uuid::Uuid::now_v7());
    let draft = |text: &str| LibraryDraft {
        id,
        name: NoteTitle::new("Fireball").unwrap(),
        facets: Facets::Spell {
            level: 3,
            school: "Evocation".into(),
            classes: vec!["Wizard".into()],
            ritual: false,
            concentration: false,
        },
        text: NoteBody::new(text.into()).unwrap(),
        book: Some(NoteTitle::new("Table rules").unwrap()),
        rules: RulesVersion::R2024,
    };
    a.library_save(draft("Our house fireball."), 0).unwrap();
    wait_for("bob sees alice's entry", || {
        search(&b, "fireball", vec![])
            .iter()
            .any(|h| h.source == "library")
    })
    .await;
    let hit = search(&b, "fireball", vec![])
        .into_iter()
        .find(|h| h.name == "Fireball")
        .unwrap();
    assert_eq!(hit.source, "library", "library entries win name clashes");
    assert_eq!(hit.others, 1);

    a.library_save(draft("Edited: house fireball, now with sparks."), 1)
        .unwrap();
    wait_for("bob gets the edit", || {
        b.compendium_entry("library", &id.to_string())
            .unwrap()
            .is_some_and(|(e, _)| {
                e.text.contains("sparks") && e.library.map(|m| m.version) == Some(2)
            })
    })
    .await;

    let hidden = search(&b, "fireball", vec![id.to_string()]);
    let shown = hidden.iter().find(|h| h.name == "Fireball").unwrap();
    assert_eq!(
        shown.source, "srd-5.2",
        "a hidden library entry falls back to the SRD"
    );

    let file = "format: beholder-library\nversion: 1\nentries:\n  - kind: condition\n    name: Dazed\n    text: Can't take reactions.\n  - kind: spell\n    name: Fireball\n    book: Table rules\n    level: 3\n    school: Evocation\n    text: dup\n  - kind: monster\n    name: Broken\n    text: no cr\n";
    let summary = a.library_import(file, FileFormat::Yaml).unwrap();
    assert_eq!(summary.added, 1);
    assert_eq!(
        summary.skipped, 1,
        "same kind, name, book and rules as an existing entry"
    );
    assert_eq!(summary.invalid.len(), 1, "{:?}", summary.invalid);
    wait_for("the imported entry reaches bob", || {
        search(&b, "dazed", vec![])
            .iter()
            .any(|h| h.source == "library")
    })
    .await;
    let exported = b.library_export(FileFormat::Json).unwrap();
    assert!(exported.contains("\"Dazed\"") && exported.contains("sparks"));
    a.shutdown();
    b.shutdown();
}
