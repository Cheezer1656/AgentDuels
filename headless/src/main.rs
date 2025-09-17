use core::panic;
use std::io::Read;

use agentduels::{client::GameConnection, SERVER_ADDR};
use agentduels_protocol::Packet;

fn main() {
    let mut connection = GameConnection::connect(SERVER_ADDR).unwrap();

    loop {
        let mut buf = [0_u8; 1024];
        connection.socket.read(&mut buf).unwrap();
        let Packet::PlayerActions(ref actions) = connection.codec.read(&buf).unwrap()[0] else {
            panic!("Expected PlayerActions packet");
        };
        println!("{:?}", actions.prev_actions);

        connection.send_packet(Packet::PlayerActions(actions.clone())).unwrap();
    }
}
