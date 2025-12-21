use crate::messages::WsMessage;
use futures::{SinkExt, StreamExt};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use tokio::sync::broadcast;
use warp::Filter;

pub fn ws_routes(
    tx: broadcast::Sender<WsMessage>,
    is_connected: Arc<AtomicBool>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let tx = warp::any().map(move || tx.clone());
    let is_connected = warp::any().map(move || is_connected.clone());

    warp::path("ws")
        .and(warp::ws())
        .and(tx)
        .and(is_connected)
        .map(|ws: warp::ws::Ws, tx, is_connected| {
            ws.on_upgrade(move |socket| handle_connection(socket, tx, is_connected))
        })
}

async fn handle_connection(
    ws: warp::ws::WebSocket,
    tx: broadcast::Sender<WsMessage>,
    is_connected: Arc<AtomicBool>,
) {
    let (mut ws_tx, _) = ws.split();
    let mut rx = tx.subscribe();

    println!("New WebSocket client connected");

    // Send initial status
    let status = if is_connected.load(Ordering::SeqCst) {
        "connected"
    } else {
        "disconnected"
    };
    
    let initial_msg = WsMessage::Status {
        event: status.to_string(),
    };

    if let Ok(json) = serde_json::to_string(&initial_msg) {
        if let Err(e) = ws_tx.send(warp::ws::Message::text(json)).await {
            eprintln!("WebSocket send error (initial status): {}", e);
            return;
        }
    }

    while let Ok(msg) = rx.recv().await {
        if let Ok(json) = serde_json::to_string(&msg) {
            if let Err(e) = ws_tx.send(warp::ws::Message::text(json)).await {
                // Ignore broken pipe or connection reset errors, as they just mean the client disconnected
                if is_disconnect_error(&e) {
                    break;
                }
                eprintln!("WebSocket send error: {}", e);
                break;
            }
        }
    }
    println!("WebSocket client disconnected");
}

fn is_disconnect_error(e: &warp::Error) -> bool {
    let msg = e.to_string();
    // println!("DEBUG: Checking error: '{}'", msg);
    msg.contains("Broken pipe") || msg.contains("Connection reset") || msg.contains("os error 32") || msg.contains("os error 54")
}
