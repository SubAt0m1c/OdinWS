use warp::{ws, Filter};
use std::sync::Arc;
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use tokio::sync::broadcast;
use tokio::sync::broadcast::Sender;
use warp::path::param;
use warp::ws::{Message, WebSocket, Ws};

type LobbyMap = Arc<DashMap<String, Sender<String>>>;

#[tokio::main]
async fn main() {
    let lobbies: LobbyMap = Arc::new(DashMap::new());

    let lobbies_filter = warp::any().map(move || lobbies.clone());
    let ws_route = param::<String>()
        .and(ws())
        .and(lobbies_filter)
        .map(|lobby_id: String, ws: Ws, lobbies: LobbyMap| {
            ws.on_upgrade(move |socket| handle_client(socket, lobby_id, lobbies))
        });

    warp::serve(ws_route).run(([127, 0, 0, 1], 3030)).await;
}

pub async fn handle_client(ws: WebSocket, lobby_id: String, lobbies: LobbyMap) {
    let (mut tx_ws, mut rx_ws) = ws.split();

    let tx = lobbies.entry(lobby_id.clone()).or_insert_with(|| {
        // println!("New lobby: {}", lobby_id);
        broadcast::channel::<String>(100).0
    }).value_mut().clone();

    let mut rx = tx.subscribe();

    // println!("New client connected: {}", lobby_id);

    loop {
        tokio::select! {
            msg = rx_ws.next() => {
                match msg {
                    Some(Ok(m)) if m.is_text() => {
                        // println!("Received in {}: {}", lobby_id, m.to_str().unwrap());
                        let _ = tx.send(m.to_str().unwrap().to_owned());
                    }
                    Some(Ok(_)) => {}
                    _ => break
                }
            }

            result = rx.recv() => {
                match result {
                    Ok(msg) => {
                        if tx_ws.send(Message::text(msg)).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {},
                    Err(_) => break
                }
            }
        }
    }

    drop(rx);
    if tx.receiver_count() == 0 {
        // println!("closing lobby: {}", lobby_id);
        lobbies.remove(&lobby_id);
    }
}
