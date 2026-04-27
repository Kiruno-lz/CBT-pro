//! WebSocket gateway for real-time engine state streaming.
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use engine::{EngineSnapshot, BacktestResult};

use crate::AppState;

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ClientMessage {
    #[serde(rename = "subscribe")]
    Subscribe { channel: String, backtest_id: String },
    #[serde(rename = "control")]
    Control { action: String, speed: Option<f64> },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum ServerMessage {
    #[serde(rename = "snapshot")]
    Snapshot { data: EngineSnapshot },
    #[serde(rename = "bar_update")]
    BarUpdate { bar: Value },
    #[serde(rename = "trade")]
    Trade { fill: Value },
    #[serde(rename = "signal")]
    Signal { signal: Value },
    #[serde(rename = "complete")]
    Complete { result: BacktestResult },
    #[serde(rename = "error")]
    Error { message: String },
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut current_backtest_id: Option<String> = None;
    let mut playback_speed: f64 = 1.0;

    while let Some(msg) = socket.recv().await {
        let msg = match msg {
            Ok(m) => m,
            Err(_) => break,
        };

        match msg {
            Message::Text(text) => {
                match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(ClientMessage::Subscribe { channel, backtest_id }) => {
                        if channel == "backtest_state" {
                            current_backtest_id = Some(backtest_id.clone());
                            // Send initial snapshot
                            let engines = state.lock().await;
                            if let Some(engine) = engines.get(&backtest_id) {
                                let snapshot = engine.get_state();
                                let resp = ServerMessage::Snapshot { data: snapshot };
                                let _ = socket.send(Message::Text(serde_json::to_string(&resp).unwrap())).await;
                            }
                        }
                    }
                    Ok(ClientMessage::Control { action, speed }) => {
                        if let Some(s) = speed {
                            playback_speed = s;
                        }
                        // Handle play/pause/step controls
                        if let Some(ref bt_id) = current_backtest_id {
                            let mut engines = state.lock().await;
                            if let Some(engine) = engines.get_mut(bt_id) {
                                match action.as_str() {
                                    "step_forward" => {
                                        if let Some(snapshot) = engine.step() {
                                            // Send snapshot
                                            let resp = ServerMessage::Snapshot { data: snapshot.clone() };
                                            let _ = socket.send(Message::Text(serde_json::to_string(&resp).unwrap())).await;
                                            // Send bar_update
                                            let bar_resp = ServerMessage::BarUpdate { bar: serde_json::to_value(&snapshot.current_bar).unwrap() };
                                            let _ = socket.send(Message::Text(serde_json::to_string(&bar_resp).unwrap())).await;
                                        }
                                    }
                                    "play" => {
                                        // Run one step as demo; full play loop would require task spawning
                                        if let Some(snapshot) = engine.step() {
                                            // Send snapshot
                                            let resp = ServerMessage::Snapshot { data: snapshot.clone() };
                                            let _ = socket.send(Message::Text(serde_json::to_string(&resp).unwrap())).await;
                                            // Send bar_update
                                            let bar_resp = ServerMessage::BarUpdate { bar: serde_json::to_value(&snapshot.current_bar).unwrap() };
                                            let _ = socket.send(Message::Text(serde_json::to_string(&bar_resp).unwrap())).await;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let resp = ServerMessage::Error { message: format!("Invalid message: {}", e) };
                        let _ = socket.send(Message::Text(serde_json::to_string(&resp).unwrap())).await;
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
}
