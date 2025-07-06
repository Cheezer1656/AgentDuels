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
    pub fn write<P: Packet>(&self, packet: &P) -> anyhow::Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(1);
        buf.push(P::ID);
        buf = postcard::to_extend(packet, buf)?;
        if let Some(cipher) = &self.cipher {
            let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
            let mut new_buf = nonce.to_vec();
            new_buf.extend(
                cipher
                    .encrypt(&nonce, buf.as_slice())
                    .map_err(|_| anyhow::anyhow!("Encryption failed"))?,
            );
            buf = new_buf;
        }
        Ok(buf)
    }

    pub fn read<P: Packet>(&self, data: &[u8]) -> anyhow::Result<P> {
        let buf = if let Some(cipher) = &self.cipher {
            let nonce = data
                .get(0..12)
                .ok_or_else(|| anyhow::anyhow!("Data too short for nonce"))?;
            cipher
                .decrypt(nonce.into(), &data[12..])
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
    fn test_packet_codec() {
        let packet = TestPacket {
            num: 42,
            data: "Hello, World!".to_string(),
        };

        let codec = PacketCodec::default();
        let serialized = codec.write(&packet).unwrap();
        let deserialized: TestPacket = codec.read(&serialized).unwrap();

        assert_eq!(packet.num, deserialized.num);
        assert_eq!(packet.data, deserialized.data);
    }

    #[test]
    fn test_packet_codec_encrypted() {
        let packet = TestPacket {
            num: 42,
            data: "Hello, World!".to_string(),
        };

        let mut codec = PacketCodec::default();
        codec.enable_encryption(&[42; 32]).unwrap();
        let serialized = codec.write(&packet).unwrap();
        let deserialized: TestPacket = codec.read(&serialized).unwrap();

        assert_eq!(packet.num, deserialized.num);
        assert_eq!(packet.data, deserialized.data);
    }
}
