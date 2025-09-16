use aes_gcm::{
    AeadCore, Aes256Gcm, KeyInit,
    aead::{Aead, OsRng},
};
use serde::{Deserialize, Serialize};

pub mod packets;

#[derive(Serialize, Deserialize)]
pub enum Packet {
    MatchID(packets::MatchIDPacket),
    Handshake(packets::HandshakePacket)
}

#[derive(Default)]
pub struct PacketCodec {
    cipher: Option<Aes256Gcm>,
}

impl PacketCodec {
    pub fn write(&self, packet: &Packet) -> anyhow::Result<Vec<u8>> {
        let mut buf = postcard::to_allocvec(packet)?;
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

    pub fn read(&self, data: &[u8]) -> anyhow::Result<Packet> {
        let buf: &[u8] = if let Some(cipher) = &self.cipher {
            let nonce = data
                .get(0..12)
                .ok_or_else(|| anyhow::anyhow!("Data too short for nonce"))?;
            &cipher
                .decrypt(nonce.into(), &data[12..])
                .map_err(|_| anyhow::anyhow!("Decryption failed"))?
        } else {
            data
        };
        Ok(postcard::from_bytes(buf)?)
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
    use super::*;

    #[test]
    fn packet_enum_size_check() {
        use std::mem::size_of;
        // Ensure the Packet enum is not larger than 16 bytes
        assert!(size_of::<Packet>() <= 16, "Packet enum is too large");
    }

    #[test]
    fn packet_encode_decode() {
        let version = 20395;
        let packet = Packet::Handshake(packets::HandshakePacket {
            protocol_version: version,
        });

        let codec = PacketCodec::default();
        let serialized = codec.write(&packet).unwrap();
        let deserialized = codec.read(&serialized).unwrap();

        match deserialized {
            Packet::Handshake(h) => assert_eq!(h.protocol_version, version),
            _ => panic!("Expected Handshake packet"),
        }
    }

    #[test]
    fn packet_encode_decode_encrypted() {
        let version = 20395;
        let packet = Packet::Handshake(packets::HandshakePacket {
            protocol_version: version,
        });

        let mut codec = PacketCodec::default();
        codec.enable_encryption(&[42; 32]).unwrap();
        let serialized = codec.write(&packet).unwrap();
        let deserialized = codec.read(&serialized).unwrap();

        match deserialized {
            Packet::Handshake(h) => assert_eq!(h.protocol_version, version),
            _ => panic!("Expected Handshake packet"),
        }
    }
}
