use agentduels_protocol::{PacketCodec, packets::HandshakePacket};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

#[tokio::main]
async fn main() {
    let mut socket = TcpStream::connect(("127.0.0.1", 8081))
        .await
        .expect("Failed to connect to game server");

    let mut codec = PacketCodec::default();

    let packet = HandshakePacket {
        protocol_version: 1,
    };
    socket
        .write_all(&codec.write(&packet).unwrap())
        .await
        .unwrap();

    let mut buf = [0; 64];
    socket.read(buf.as_mut_slice()).await.unwrap();
    let packet: HandshakePacket = codec.read(&buf).unwrap();

    println!(
        "Other client has protocol version {}",
        packet.protocol_version
    );
}
