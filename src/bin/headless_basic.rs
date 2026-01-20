#![feature(mpmc_channel)]

use agentduels::player::PlayerActions;
use agentduels::{SERVER_URL, TickMessage, client::GameConnection};
use anyhow::bail;
use workflow_websocket::client::Message;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let connection = GameConnection::connect(SERVER_URL).await?;

    loop {
        let msg = connection.receiver_rx.recv()?;
        let Message::Binary(data) = msg else {
            if msg == Message::Close {
                println!("Connection closed by server");
                return Ok(());
            }
            bail!("Received invalid message: {:?}", msg);
        };
        let tick_msg: TickMessage = postcard::from_bytes(&data)?;
        println!("Received tick {}", tick_msg.tick);

        let mut actions = PlayerActions::default();
        actions.set(PlayerActions::MOVE_FORWARD);
        actions.set(PlayerActions::JUMP);
        actions.set(PlayerActions::ATTACK);

        connection
            .socket
            .send(Message::Binary(
                postcard::to_allocvec(&actions).unwrap(),
            ))
            .await?;
    }
}
