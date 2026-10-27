use std::sync::Arc;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket};
use beholder_core::{CampaignId, Role, Seq, UserId};
use beholder_protocol::{
    ClientMsg, Command, CommandKind, Hello, HelloError, HelloOk, PROTOCOL_VERSION, RejectReason,
    ServerMsg, codec,
};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::time::{Instant, sleep_until, timeout};

use crate::campaigns::{self, CatchUp, Handled, Reply};
use crate::db::DbError;
use crate::hub::{ConnId, QUEUE_FRAMES};
use crate::permissions::Outgoing;
use crate::{AppState, auth, now};

pub const IDLE_TIMEOUT: Duration = Duration::from_secs(35);

const HELLO_TIMEOUT: Duration = Duration::from_secs(10);

pub const PRESENCE_GRACE: Duration = Duration::from_secs(10);

pub async fn run(mut socket: WebSocket, state: AppState) {
    let Some(hello) = read_hello(&mut socket).await else {
        return;
    };
    if hello.protocol_version != PROTOCOL_VERSION {
        reject(
            socket,
            HelloError::UpdateRequired {
                server_version: PROTOCOL_VERSION,
            },
        )
        .await;
        return;
    }
    let token = hello.session_token.clone();
    let session = state
        .db
        .call(move |c| auth::validate_session(c, &token, now(), true))
        .await;
    let session = match session {
        Ok(Some(s)) => s,
        Ok(None) => return reject(socket, HelloError::InvalidSession).await,
        Err(e) => {
            tracing::error!("session check failed: {e}");
            return;
        }
    };
    let user_id = session.user.id;
    let (tx, rx) = mpsc::channel::<Bytes>(QUEUE_FRAMES);
    let (conn_id, announce) = state.hub.register(user_id, tx);

    let st = Arc::clone(&state);
    let opened = state
        .db
        .call(move |c| open_session(c, &st, conn_id, session, &hello, announce))
        .await;
    if let Err(e) = opened {
        tracing::error!("session open failed: {e}");
    } else {
        let (sink, stream) = socket.split();
        let mut writer = tokio::spawn(write_loop(sink, rx));
        read_loop(stream, &state, conn_id, user_id, &mut writer).await;
        writer.abort();
    }

    if state.hub.unregister(conn_id, user_id) {
        tokio::spawn(announce_offline_later(state, user_id));
    }
}

async fn read_hello(socket: &mut WebSocket) -> Option<Hello> {
    let msg = timeout(HELLO_TIMEOUT, socket.recv()).await.ok()??.ok()?;
    let Message::Binary(frame) = msg else {
        return None;
    };
    match codec::decode(&frame) {
        Ok(ClientMsg::Hello(h)) => Some(h),
        _ => None,
    }
}

async fn reject(mut socket: WebSocket, err: HelloError) {
    if let Ok(frame) = codec::encode(&ServerMsg::HelloErr(err)) {
        let _ = socket.send(Message::Binary(frame.into())).await;
    }
    let _ = socket.send(Message::Close(None)).await;
}

fn open_session(
    conn: &mut rusqlite::Connection,
    state: &AppState,
    conn_id: ConnId,
    session: auth::Session,
    hello: &Hello,
    announce: bool,
) -> Result<(), DbError> {
    let hub = &state.hub;
    let memberships = campaigns::memberships_of(conn, session.user.id)?;
    let ok = ServerMsg::HelloOk(HelloOk {
        user: session.user.clone(),
        campaigns: memberships.iter().map(|(id, _)| *id).collect(),
        rotated_token: session.rotated,
    });
    if let Some(out) = Outgoing::control(ok) {
        hub.send(conn_id, &out);
    }
    for (campaign_id, role) in memberships {
        if announce {
            hub.broadcast(
                campaign_id,
                &Outgoing::presence_changed(campaign_id, session.user.id, true),
            );
        }
        let Some((seq, campaign)) = campaigns::load_campaign(conn, campaign_id)? else {
            continue;
        };
        let catch_up = match known_seq(hello, campaign_id) {
            Some(known) => campaigns::catch_up(conn, campaign_id, known)?,
            None => CatchUp::Snapshot,
        };
        match catch_up {
            CatchUp::Events(events) => {
                if let Some(out) = Outgoing::events(role, campaign_id, &events) {
                    hub.send(conn_id, &out);
                }
            }
            CatchUp::Snapshot => hub.send(conn_id, &Outgoing::snapshot(role, seq, &campaign)),
        }
        hub.subscribe(conn_id, campaign_id, role);
        hub.send(conn_id, &hub.presence(&campaign));
    }
    Ok(())
}

fn known_seq(hello: &Hello, id: CampaignId) -> Option<Seq> {
    hello.known.iter().find(|(c, _)| *c == id).map(|(_, s)| *s)
}

async fn write_loop(mut sink: SplitSink<WebSocket, Message>, mut rx: mpsc::Receiver<Bytes>) {
    while let Some(frame) = rx.recv().await {
        match timeout(IDLE_TIMEOUT, sink.send(Message::Binary(frame))).await {
            Ok(Ok(())) => {}
            _ => return,
        }
    }
    let _ = sink.send(Message::Close(None)).await;
}

async fn read_loop(
    mut stream: SplitStream<WebSocket>,
    state: &AppState,
    conn_id: ConnId,
    user_id: UserId,
    writer: &mut tokio::task::JoinHandle<()>,
) {
    let mut deadline = Instant::now() + IDLE_TIMEOUT;
    loop {
        let msg = tokio::select! {
            incoming = stream.next() => match incoming {
                Some(Ok(msg)) => msg,
                _ => return,
            },
            _ = &mut *writer => return,
            () = sleep_until(deadline) => return,
        };
        deadline = Instant::now() + IDLE_TIMEOUT;
        let frame = match msg {
            Message::Binary(f) => f,
            Message::Ping(_) | Message::Pong(_) => continue,
            Message::Close(_) | Message::Text(_) => return,
        };
        match codec::decode::<ClientMsg>(&frame) {
            Ok(ClientMsg::Ping { t }) => {
                if let Some(out) = Outgoing::control(ServerMsg::Pong { t }) {
                    state.hub.send(conn_id, &out);
                }
            }
            Ok(ClientMsg::Command(cmd)) => {
                if let Err(e) = handle_command(state, conn_id, user_id, cmd).await {
                    tracing::error!("command failed: {e}");
                    return;
                }
            }
            Ok(ClientMsg::Hello(_)) | Err(_) => return,
        }
    }
}

async fn handle_command(
    state: &AppState,
    conn_id: ConnId,
    user_id: UserId,
    cmd: Command,
) -> Result<(), DbError> {
    let id = cmd.id;
    let send_reply = |reply: Reply| {
        let msg = match reply {
            Reply::Ack(seq) => ServerMsg::Ack { id, seq },
            Reply::Reject(reason) => ServerMsg::Reject { id, reason },
        };
        if let Some(out) = Outgoing::control(msg) {
            state.hub.send(conn_id, &out);
        }
    };
    if matches!(cmd.kind, CommandKind::JoinCampaign { .. }) && !state.join_limit.allow(user_id) {
        send_reply(Reply::Reject(RejectReason::RateLimited));
        return Ok(());
    }
    let st = Arc::clone(state);
    let reply = state
        .db
        .call(move |c| {
            let Some(user) = crate::auth::user_by_id(c, user_id)? else {
                return Ok(Reply::Reject(RejectReason::NotFound));
            };
            let handled = campaigns::handle_command(c, &user, &cmd, now())?;
            publish(c, &st, &handled, user_id)?;
            Ok::<_, DbError>(handled.reply)
        })
        .await?;
    send_reply(reply);
    Ok(())
}

fn publish(
    conn: &rusqlite::Connection,
    state: &AppState,
    handled: &Handled,
    actor: UserId,
) -> Result<(), DbError> {
    let Some(campaign_id) = handled.campaign_id else {
        return Ok(());
    };
    // Events go to existing subscribers first, so the new member's snapshot already includes them.
    state.hub.publish_events(campaign_id, &handled.events);
    if let Some(role) = handled.joined
        && let Some((seq, campaign)) = campaigns::load_campaign(conn, campaign_id)?
    {
        state.hub.add_member(actor, role, seq, &campaign);
        if role == Role::Player {
            state.hub.broadcast(
                campaign_id,
                &Outgoing::presence_changed(campaign_id, actor, true),
            );
        }
    }
    Ok(())
}

async fn announce_offline_later(state: AppState, user: UserId) {
    tokio::time::sleep(PRESENCE_GRACE).await;
    if !state.hub.expire_presence(user) {
        return;
    }
    let memberships = state
        .db
        .call(move |c| campaigns::memberships_of(c, user))
        .await;
    match memberships {
        Ok(list) => {
            for (campaign_id, _) in list {
                state.hub.broadcast(
                    campaign_id,
                    &Outgoing::presence_changed(campaign_id, user, false),
                );
            }
        }
        Err(e) => tracing::warn!("presence lookup failed: {e}"),
    }
}
