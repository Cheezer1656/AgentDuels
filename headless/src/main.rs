use core::panic;
use std::io::Read;

use agentduels::{client::GameConnection, SERVER_ADDR};
use agentduels_protocol::Packet;

fn main() {
    let mut connection = GameConnection::connect(SERVER_ADDR).unwrap();

    loop {
        let mut buf = [0_u8; 1024];
        connection.socket.read(&mut buf).unwrap();
        let packets = connection.codec.read(&buf).unwrap();
        let Some(Packet::PlayerActions(actions)) = packets.get(0) else {
            println!("Expected PlayerActions packet");
            break;
        };
        println!("{:?}", actions.prev_actions);

        connection.send_packet(Packet::PlayerActions(actions.clone())).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
