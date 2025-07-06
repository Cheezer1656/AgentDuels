use aes_gcm::{
    AeadCore, Aes256Gcm, KeyInit,
    aead::{Aead, OsRng},
};
use serde::{Deserialize, Serialize};

pub mod packets;

pub trait Packet: Serialize + for<'de> Deserialize<'de> + Send + Sync {
    const ID: u8;
}

#[derive(Default)]
pub struct PacketCodec {
    cipher: Option<Aes256Gcm>,
}

impl PacketCodec {
    pub fn write<P: Packet>(&mut self, packet: &P) -> anyhow::Result<Vec<u8>> {
        let serialized = postcard::to_allocvec(packet)?;
        let mut buf = Vec::with_capacity(serialized.len() + 1);
        buf.push(P::ID);
        if let Some(cipher) = &self.cipher {
            let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
            buf = cipher
                .encrypt(&nonce, serialized.as_slice())
                .map_err(|_| anyhow::anyhow!("Encryption failed"))?;
        } else {
            buf.extend(serialized);
        }
        Ok(buf)
    }

    pub fn read<P: Packet>(&mut self, data: &[u8]) -> anyhow::Result<P> {
        let buf = if let Some(cipher) = &self.cipher {
            let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
            cipher
                .decrypt(&nonce, data)
                .map_err(|_| anyhow::anyhow!("Decryption failed"))?
        } else {
            data.to_vec()
        };
        if buf.is_empty() || buf[0] != P::ID {
            return Err(anyhow::anyhow!("Invalid packet ID"));
        }
        let packet: P = postcard::from_bytes(&buf[1..])?;
        Ok(packet)
    }

    pub fn enable_encryption(&mut self, key: &[u8; 32]) -> anyhow::Result<()> {
        self.cipher = Some(
            Aes256Gcm::new_from_slice(key)
                .map_err(|_| anyhow::anyhow!("Invalid encryption key"))?,
        );
        Ok(())
    }
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
