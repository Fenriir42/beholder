use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Context;
use beholder_server::{Server, serve};
use tracing::Level;

fn main() -> anyhow::Result<()> {
    let level: Level = std::env::var("BEHOLDER_LOG")
        .ok()
        .and_then(|l| l.parse().ok())
        .unwrap_or(Level::INFO);
    tracing_subscriber::fmt().with_max_level(level).init();

    let bind: SocketAddr = std::env::var("BEHOLDER_BIND")
        .unwrap_or_else(|_| "0.0.0.0:8080".into())
        .parse()
        .context("BEHOLDER_BIND")?;
    let data_dir =
        PathBuf::from(std::env::var("BEHOLDER_DATA_DIR").unwrap_or_else(|_| "data-dev".into()));
    std::fs::create_dir_all(&data_dir).context("creating data dir")?;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(8)
        .enable_all()
        .build()?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        return rt.block_on(admin(&data_dir, &args));
    }
    rt.block_on(async {
        let docs_dir = std::env::var_os("BEHOLDER_DOCS_DIR")
            .map(PathBuf::from)
            .filter(|p| p.is_dir());
        let state = Server::open(data_dir, docs_dir).context("starting server")?;
        let listener = tokio::net::TcpListener::bind(bind)
            .await
            .context("binding")?;
        tracing::info!(%bind, "listening");
        if let Ok(repo) = std::env::var("BEHOLDER_IMPORT_REPO")
            && !repo.trim().is_empty()
        {
            let git_ref = std::env::var("BEHOLDER_IMPORT_REF").unwrap_or_else(|_| "HEAD".into());
            tokio::spawn(beholder_server::setup_import::run(
                std::sync::Arc::clone(&state),
                repo,
                git_ref,
            ));
        }
        serve(listener, state, shutdown_signal()).await?;
        Ok(())
    })
}

async fn admin(data_dir: &std::path::Path, args: &[String]) -> anyhow::Result<()> {
    use beholder_server::{auth, db::Db, now};
    let db = Db::open(&data_dir.join("beholder.db")).context("opening the database")?;
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["users"] => {
            let list = db.call(|c| auth::accounts(c)).await?;
            println!(
                "{:<24} {:<24} {:>8}",
                "username", "display name", "sessions"
            );
            for a in list {
                println!(
                    "{:<24} {:<24} {:>8}",
                    a.username, a.display_name, a.sessions
                );
            }
            Ok(())
        }
        ["reset-password", name] => {
            let password = auth::random_password().map_err(|e| anyhow::anyhow!("{e:?}"))?;
            let phc = auth::hash_password(&password).map_err(|e| anyhow::anyhow!("{e:?}"))?;
            let name = (*name).to_owned();
            let target = name.clone();
            let found = db
                .call(move |c| auth::reset_password(c, &target, &phc, now()))
                .await?;
            if !found {
                anyhow::bail!("no user named {name}");
            }
            println!("New password for {name}: {}", password.expose());
            println!("All of {name}'s sessions were signed out.");
            Ok(())
        }
        ["backup", rest @ ..] => {
            let keep = rest
                .first()
                .map_or(Ok(14), |k| k.parse::<usize>())
                .context("keep")?;
            let dir = data_dir.join("backups");
            let stamp = u64::try_from(now().0 / 1000).unwrap_or(0);
            let path = db
                .call(move |c| beholder_server::db::backup(c, &dir, stamp, keep))
                .await?;
            println!("Backed up to {}", path.display());
            Ok(())
        }
        _ => anyhow::bail!(
            "usage: beholder-server [users | reset-password <username> | backup [keep]]"
        ),
    }
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let term = async {
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(mut s) => {
                    s.recv().await;
                }
                Err(_) => std::future::pending().await,
            }
        };
        tokio::select! { _ = ctrl_c => {}, () = term => {} }
    }
    #[cfg(not(unix))]
    {
        let _ = ctrl_c.await;
    }
}
