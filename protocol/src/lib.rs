use serde::{Deserialize, Serialize};

pub mod packets;

pub trait Packet: Serialize + for<'de> Deserialize<'de> + Send + Sync {
    const ID: u8;
}

#[cfg(test)]
mod tests {
    use agentduels_protocol_macros::Packet;

    use super::*;

    #[derive(Packet, Serialize, Deserialize)]
    struct TestPacket {
        num: u32,
        data: String,
    }

    #[test]
    fn test_packet_serialize_deserialize() {
        let packet = TestPacket {
            num: 42,
            data: "Hello, World!".to_string(),
        };

        let serialized = postcard::to_allocvec(&packet).unwrap();
        let deserialized: TestPacket = postcard::from_bytes(&serialized).unwrap();

        assert_eq!(packet.num, deserialized.num);
        assert_eq!(packet.data, deserialized.data);
    }
}