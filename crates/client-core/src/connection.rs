use std::time::Duration;

use beholder_core::CampaignEvent;
use beholder_protocol::{
    ClientMsg, Command, Hello, HelloError, PROTOCOL_VERSION, ServerMsg, SessionToken, codec,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::time::{Instant, sleep_until, timeout};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::{ClientEvent, ConnectionState, Shared};

pub const PING_EVERY: Duration = Duration::from_secs(15);

pub const IDLE_TIMEOUT: Duration = Duration::from_secs(35);

const BACKOFF_START: Duration = Duration::from_millis(500);

const BACKOFF_CAP: Duration = Duration::from_secs(10);

const OFFLINE_AFTER: u32 = 3;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) enum Wake {
    Send(Command),
    NetworkChanged,
    Shutdown,
}

enum End {
    Shutdown,
    Fatal(ClientEvent),
    Lost { online: bool },
}

pub(crate) async fn run(
    shared: std::sync::Arc<Shared>,
    ws_url: String,
    mut token: SessionToken,
    mut rx: mpsc::UnboundedReceiver<Wake>,
) {
    let mut failures = 0u32;
    let mut ever_online = false;
    loop {
        let queued = shared.queued();
        shared.set_state(if failures >= OFFLINE_AFTER {
            ConnectionState::Offline { queued }
        } else if ever_online {
            ConnectionState::Reconnecting { queued }
        } else {
            ConnectionState::Connecting
        });
        match session(&shared, &ws_url, &mut token, &mut rx).await {
            End::Shutdown => return,
            End::Fatal(event) => {
                shared.set_state(ConnectionState::Offline {
                    queued: shared.queued(),
                });
                shared.emit(event);
                while let Some(w) = rx.recv().await {
                    if matches!(w, Wake::Shutdown) {
                        return;
                    }
                }
                return;
            }
            End::Lost { online } => {
                if online {
                    ever_online = true;
                    failures = 1;
                } else {
                    failures = failures.saturating_add(1);
                }
            }
        }
        if failures >= OFFLINE_AFTER {
            shared.set_state(ConnectionState::Offline {
                queued: shared.queued(),
            });
        }
        let deadline = Instant::now() + backoff(failures);
        loop {
            tokio::select! {
                () = sleep_until(deadline) => break,
                w = rx.recv() => match w {
                    None | Some(Wake::Shutdown) => return,
                    Some(Wake::NetworkChanged) => break,
                    Some(Wake::Send(_)) => {
                        let queued = shared.queued();
                        shared.set_state(if failures >= OFFLINE_AFTER {
                            ConnectionState::Offline { queued }
                        } else {
                            ConnectionState::Reconnecting { queued }
                        });
                    }
                },
            }
        }
    }
}

pub fn backoff(failures: u32) -> Duration {
    let exp = BACKOFF_START.saturating_mul(1 << failures.saturating_sub(1).min(16));
    let base = exp.min(BACKOFF_CAP);
    let half = base / 2;
    half + half.mul_f64(fastrand::f64())
}

async fn session(
    shared: &std::sync::Arc<Shared>,
    ws_url: &str,
    token: &mut SessionToken,
    rx: &mut mpsc::UnboundedReceiver<Wake>,
) -> End {
    let lost = End::Lost { online: false };
    let Ok(Ok((mut ws, _))) = timeout(HANDSHAKE_TIMEOUT, connect_async(ws_url)).await else {
        return lost;
    };
    let Ok(known) = shared.with_cache(|c| c.known()) else {
        return lost;
    };
    let hello = ClientMsg::Hello(Hello {
        protocol_version: PROTOCOL_VERSION,
        session_token: token.clone(),
        known,
    });
    let Ok(frame) = codec::encode(&hello) else {
        return lost;
    };
    if ws.send(Message::Binary(frame.into())).await.is_err() {
        return lost;
    }

    let first = timeout(HANDSHAKE_TIMEOUT, async {
        loop {
            match ws.next().await {
                Some(Ok(Message::Binary(f))) => return codec::decode::<ServerMsg>(&f).ok(),
                Some(Ok(_)) => continue,
                _ => return None,
            }
        }
    })
    .await;
    let ok = match first {
        Ok(Some(ServerMsg::HelloOk(ok))) => ok,
        Ok(Some(ServerMsg::HelloErr(HelloError::InvalidSession))) => {
            return End::Fatal(ClientEvent::SessionInvalid);
        }
        Ok(Some(ServerMsg::HelloErr(HelloError::UpdateRequired { server_version }))) => {
            return End::Fatal(ClientEvent::UpdateRequired { server_version });
        }
        _ => return lost,
    };
    if let Some(new) = ok.rotated_token {
        *token = new.clone();
        shared.emit(ClientEvent::SessionRotated(new));
    }
    let synced = shared.with_cache(|c| {
        c.set_user(&ok.user)?;
        c.retain_campaigns(&ok.campaigns)
    });
    if synced.is_err() {
        return lost;
    }
    shared.clear_presence();
    shared.set_state(ConnectionState::Online);
    shared.emit(ClientEvent::ViewChanged);
    crate::compendium::maybe_start(shared, token);

    let online = End::Lost { online: true };
    let Ok(outbox) = shared.with_cache(|c| c.outbox()) else {
        return online;
    };
    for cmd in outbox {
        if send(&mut ws, &ClientMsg::Command(cmd)).await.is_err() {
            return online;
        }
    }

    let started = Instant::now();
    let mut ping = tokio::time::interval_at(Instant::now() + PING_EVERY, PING_EVERY);
    let mut deadline = Instant::now() + IDLE_TIMEOUT;
    loop {
        tokio::select! {
            msg = ws.next() => {
                let frame = match msg {
                    Some(Ok(Message::Binary(f))) => f,
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => return online,
                    Some(Ok(_)) => {
                        deadline = Instant::now() + IDLE_TIMEOUT;
                        continue;
                    }
                };
                deadline = Instant::now() + IDLE_TIMEOUT;
                let Ok(msg) = codec::decode::<ServerMsg>(&frame) else { return online };
                let sources_may_change = match &msg {
                    ServerMsg::Snapshot { .. }
                    | ServerMsg::CampaignRemoved { .. }
                    | ServerMsg::LibraryChanged { .. } => true,
                    ServerMsg::Events { events, .. } => events
                        .iter()
                        .any(|e| matches!(e.event, CampaignEvent::SourcesChanged { .. })),
                    _ => false,
                };
                if shared.handle(msg).is_err() {
                    return online;
                }
                if sources_may_change {
                    crate::compendium::maybe_start(shared, token);
                }
            }
            _ = ping.tick() => {
                let t = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                if send(&mut ws, &ClientMsg::Ping { t }).await.is_err() {
                    return online;
                }
            }
            () = sleep_until(deadline) => return online,
            w = rx.recv() => match w {
                Some(Wake::Send(cmd)) => {
                    if send(&mut ws, &ClientMsg::Command(cmd)).await.is_err() {
                        return online;
                    }
                }
                Some(Wake::NetworkChanged) => {
                    if send(&mut ws, &ClientMsg::Ping { t: 0 }).await.is_err() {
                        return online;
                    }
                    deadline = deadline.min(Instant::now() + HANDSHAKE_TIMEOUT);
                }
                None | Some(Wake::Shutdown) => {
                    let _ = ws.close(None).await;
                    return End::Shutdown;
                }
            },
        }
    }
}

async fn send<S>(ws: &mut S, msg: &ClientMsg) -> Result<(), ()>
where
    S: SinkExt<Message> + Unpin,
{
    let frame = codec::encode(msg).map_err(|_| ())?;
    timeout(IDLE_TIMEOUT, ws.send(Message::Binary(frame.into())))
        .await
        .map_err(|_| ())?
        .map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_with_jitter_and_caps() {
        for _ in 0..200 {
            let first = backoff(1);
            assert!(first >= Duration::from_millis(250) && first <= Duration::from_millis(500));
            let fourth = backoff(4);
            assert!(fourth >= Duration::from_secs(2) && fourth <= Duration::from_secs(4));
            let late = backoff(50);
            assert!(late >= Duration::from_secs(5) && late <= BACKOFF_CAP);
        }
    }
}
