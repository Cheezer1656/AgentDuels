use agentduels::player::PlayerID;
use agentduels::{GAME_VERSION, SERVER_ADDR};
use anyhow::bail;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::spawn;
use tungstenite::{Message, WebSocket, accept};

mod app;

fn main() {
    let server = TcpListener::bind(SERVER_ADDR).unwrap();
    let queue: Arc<Mutex<Option<WebSocket<TcpStream>>>> = Arc::new(Mutex::new(None));

    for stream in server.incoming() {
        let queue = queue.clone();
        spawn(move || {
            if let Ok(stream) = stream {
                let _ = handle_connection(stream, queue).unwrap();
            }
        });
    }
}

/// Reads messages from the WebSocket until a binary message is received. (Used to ignore pings.)
fn read_until_binary(ws: &mut WebSocket<TcpStream>) -> anyhow::Result<Vec<u8>> {
    loop {
        let msg = ws.read()?;
        if msg.is_binary() {
            return Ok(msg.into_data().to_vec());
        }
    }
}

fn handle_connection(
    stream: TcpStream,
    queue: Arc<Mutex<Option<WebSocket<TcpStream>>>>,
) -> anyhow::Result<()> {
    let mut websocket = accept(stream)?;

    if let Ok(data) = read_until_binary(&mut websocket) {
        if data.len() != 4 {
            bail!("Wrong data length");
        }
        let client_version = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        if client_version != GAME_VERSION {
            bail!("Version mismatch");
        }
    } else {
        bail!("Unexpected message");
    };

    let mut queue_lock = queue
        .lock()
        .map_err(|e: PoisonError<_>| anyhow::anyhow!("Mutex poisoned: {}", e))?;
    if let Some(websocket2) = queue_lock.take() {
        drop(queue_lock);
        let match_id_bytes = rand::random::<u64>().to_be_bytes();
        let mut websockets = [websocket, websocket2];
        for (player_id, ws) in websockets.iter_mut().enumerate() {
            ws.send(Message::binary(match_id_bytes.to_vec()))?;
            ws.send(Message::binary((player_id as u16).to_be_bytes().to_vec()))?;
        }
        app::start_app(websockets)?;
    } else {
        *queue_lock = Some(websocket);
    }

    Ok(())
}
