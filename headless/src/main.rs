use agentduels::{networking::GameClient, SERVER_ADDR};

fn main() {
    let client = GameClient::connect(SERVER_ADDR);
}
