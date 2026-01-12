use std::io::Read;

use agentduels::{SERVER_ADDR, client::GameConnection};
use agentduels_protocol::Packet;

fn main() {
    let mut connection = GameConnection::connect(SERVER_ADDR).unwrap();

    loop {
        let mut buf = [0_u8; 1024];
        connection.socket.read(&mut buf).unwrap();
        let Ok(Some(Packet::PlayerActions(actions))) = connection.codec.read(&buf) else {
            continue;
        };
        println!("{:?}", actions.prev_actions);

        connection
            .send_packet(Packet::PlayerActions(actions.clone()))
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
