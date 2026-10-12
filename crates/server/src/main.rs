//! Beholder server binary. Configuration comes from the environment:
//! `BEHOLDER_BIND` (default `0.0.0.0:8080`), `BEHOLDER_DATA_DIR` (default `./data-dev`),
//! `BEHOLDER_LOG` (`error`…`trace`, default `info`).

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

    // Two workers are plenty for one friend group; a small blocking pool bounds
    // the threads (and stacks) used by SQLite, Argon2 and image work.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(8)
        .enable_all()
        .build()?;
    rt.block_on(async {
        let state = Server::open(data_dir).context("opening database")?;
        let listener = tokio::net::TcpListener::bind(bind)
            .await
            .context("binding")?;
        tracing::info!(%bind, "listening");
        serve(listener, state, shutdown_signal()).await?;
        Ok(())
    })
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
