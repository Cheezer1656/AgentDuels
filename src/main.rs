use agentduels_protocol::packets::HandshakePacket;
use postcard::{from_bytes, to_allocvec};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut socket = TcpStream::connect(("127.0.0.1", 8081)).await.expect("Failed to connect to game server");

    let packet = HandshakePacket {
        protocol_version: 1
    };
    socket.write_all(&to_allocvec(&packet).unwrap()).await?;

    let mut buf = [0; 64];
    socket.read(buf.as_mut_slice()).await.unwrap();
    let packet: HandshakePacket = from_bytes(buf.as_slice()).unwrap();

    println!("Other client has protocol version {}", packet.protocol_version);

    Ok(())
}