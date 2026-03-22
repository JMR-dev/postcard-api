use crate::auth::Claims;
use crate::state::{SharedState, UserId};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::{sync::mpsc, time::timeout};
use chrono::Utc;

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum InboundFrame {
    Authenticate { token: String },
    #[serde(other)]
    Unknown,
}

#[derive(Serialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum OutboundFrame {
    PresenceUpdate {
        user_id: UserId,
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        last_seen: Option<String>,
    },
    Error {
        message: String,
    },
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: SharedState) {
    let auth_msg = match timeout(Duration::from_secs(5), socket.next()).await {
        Ok(Some(Ok(msg))) => msg,
        _ => return,
    };

    let token = if let Message::Text(text) = auth_msg {
        if let Ok(InboundFrame::Authenticate { token }) = serde_json::from_str(&text) {
            token
        } else {
            let err_json = serde_json::to_string(&OutboundFrame::Error { message: "Expected Authenticate frame".into() }).unwrap();
            let _ = socket.send(Message::Text(err_json)).await;
            return;
        }
    } else {
        return;
    };

    let user_id = match decode::<Claims>(
        &token,
        &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
        &Validation::default(),
    ) {
        Ok(token_data) => token_data.claims.sub,
        Err(_) => {
            let err_json = serde_json::to_string(&OutboundFrame::Error { message: "Invalid token".into() }).unwrap();
            let _ = socket.send(Message::Text(err_json)).await;
            return;
        }
    };

    let (tx, mut rx) = mpsc::channel::<OutboundFrame>(32);
    {
        let mut connections = state.connections.write().await;
        connections.entry(user_id).or_default().push(tx);
    }

    let presence_msg = OutboundFrame::PresenceUpdate {
        user_id,
        status: "online".to_string(),
        last_seen: None,
    };
    broadcast_all(&state, presence_msg).await;

    let (mut sender, mut receiver) = socket.split();

    let mut ping_interval = tokio::time::interval(Duration::from_secs(30));
    ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    
    let pong_timeout = tokio::time::sleep(Duration::MAX);
    tokio::pin!(pong_timeout);
    let mut awaiting_pong = false;

    loop {
        tokio::select! {
            _ = ping_interval.tick() => {
                if sender.send(Message::Ping(vec![])).await.is_err() {
                    break;
                }
                awaiting_pong = true;
                pong_timeout.as_mut().reset(tokio::time::Instant::now() + Duration::from_secs(10));
            }
            _ = &mut pong_timeout, if awaiting_pong => {
                break;
            }
            msg = rx.recv() => {
                if let Some(frame) = msg {
                    if let Ok(json) = serde_json::to_string(&frame) {
                        if sender.send(Message::Text(json)).await.is_err() {
                            break;
                        }
                    }
                } else {
                    break;
                }
            }
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(frame) = serde_json::from_str::<InboundFrame>(&text) {
                            match frame {
                                InboundFrame::Unknown | InboundFrame::Authenticate { .. } => {
                                    let err_json = serde_json::to_string(&OutboundFrame::Error { message: "Unknown or unexpected frame type".into() }).unwrap();
                                    let _ = sender.send(Message::Text(err_json)).await;
                                }
                            }
                        } else {
                            let err_json = serde_json::to_string(&OutboundFrame::Error { message: "Invalid JSON".into() }).unwrap();
                            let _ = sender.send(Message::Text(err_json)).await;
                        }
                    }
                    Some(Ok(Message::Pong(_))) => {
                        awaiting_pong = false;
                        pong_timeout.as_mut().reset(tokio::time::Instant::now() + Duration::from_secs(86400));
                    }
                    Some(Ok(Message::Close(_))) => {
                        break;
                    }
                    Some(Err(_)) | None => break,
                    _ => {}
                }
            }
        }
    }

    let mut connections = state.connections.write().await;
    let mut is_empty = false;
    if let Some(user_conns) = connections.get_mut(&user_id) {
        drop(rx);
        user_conns.retain(|tx| !tx.is_closed());
        if user_conns.is_empty() {
            is_empty = true;
        }
    }

    if is_empty {
        connections.remove(&user_id);
        drop(connections);

        let offline_msg = OutboundFrame::PresenceUpdate {
            user_id,
            status: "offline".to_string(),
            last_seen: Some(Utc::now().to_rfc3339()),
        };
        broadcast_all(&state, offline_msg).await;
    }
}

async fn broadcast_all(state: &SharedState, frame: OutboundFrame) {
    let connections = state.connections.read().await;
    for conns in connections.values() {
        for tx in conns {
            let _ = tx.send(frame.clone()).await;
        }
    }
}
