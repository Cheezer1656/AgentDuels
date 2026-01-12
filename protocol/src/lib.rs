use aes_gcm::{
    AeadCore, Aes256Gcm, KeyInit,
    aead::{Aead, OsRng},
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub mod packets;

pub use packets::Item;
pub use packets::PlayerActions;

#[derive(Serialize, Deserialize, Debug)]
pub enum Packet {
    MatchID(packets::MatchIDPacket),
    Handshake(packets::HandshakePacket),
    PlayerActions(Box<packets::PlayerActionsPacket>),
}

fn push_varint(buf: &mut Vec<u8>, mut value: u32) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn read_varint(data: &[u8]) -> Option<(u32, usize)> {
    let mut result = 0u32;
    let mut shift = 0;
    for (i, &byte) in data.iter().enumerate() {
        result |= ((byte & 0x7F) as u32) << shift;
        if byte & 0x80 == 0 {
            return Some((result, i + 1));
        }
        shift += 7;
        if shift > 35 {
            return None; // Varint is too big
        }
    }
    None // Not enough data
}

#[derive(Default)]
pub struct PacketCodec {
    cipher: Option<Aes256Gcm>,
    buffer: Vec<u8>,
    packets: VecDeque<Packet>,
}

impl PacketCodec {
    pub fn write(&self, packet: &Packet) -> anyhow::Result<Vec<u8>> {
        let mut data = postcard::to_allocvec(packet)?;
        if let Some(cipher) = &self.cipher {
            let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
            let mut new_buf = nonce.to_vec();
            new_buf.extend(
                cipher
                    .encrypt(&nonce, data.as_slice())
                    .map_err(|_| anyhow::anyhow!("Encryption failed"))?,
            );
            data = new_buf;
        }

        let mut buf = Vec::with_capacity(data.len() + 1);
        push_varint(&mut buf, data.len() as u32);
        buf.extend(&data);

        Ok(buf)
    }

    pub fn read(&mut self, data: &[u8]) -> anyhow::Result<Option<Packet>> {
        self.buffer.extend_from_slice(data);
        while let Some((length, varint_size)) = read_varint(&self.buffer) {
            if length == 0 {
                self.buffer.drain(0..1);
                continue;
            } else if self.buffer.len() < length as usize + varint_size {
                break; // Not enough data yet
            }
            self.buffer.drain(..varint_size);
            let data = self.buffer.drain(..length as usize).collect::<Vec<u8>>();

            let buf: Vec<u8> = if let Some(cipher) = &self.cipher {
                let nonce = data
                    .get(0..12)
                    .ok_or_else(|| anyhow::anyhow!("Data too short for nonce"))?;
                cipher
                    .decrypt(nonce.into(), &data[12..])
                    .map_err(|_| anyhow::anyhow!("Decryption failed"))?
            } else {
                data
            };
            self.packets.push_back(postcard::from_bytes(&buf)?);
        }
        Ok(self.packets.pop_front())
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

        let mut codec = PacketCodec::default();
        let serialized = codec.write(&packet).unwrap();
        let deserialized = codec.read(&serialized).unwrap().unwrap();

        match deserialized {
            Packet::Handshake(h) => assert_eq!(h.protocol_version, version),
            _ => panic!("Expected Handshake packet"),
        }
    }

    #[test]
    fn packet_encode_decode_multiple() {
        let mut codec = PacketCodec::default();
        let mut serialized = Vec::new();

        let id = 1024952;
        let packet = Packet::MatchID(packets::MatchIDPacket { id });
        serialized.extend(codec.write(&packet).unwrap());

        let version = 20395;
        let packet = Packet::Handshake(packets::HandshakePacket {
            protocol_version: version,
        });
        serialized.extend(codec.write(&packet).unwrap());

        match codec.read(&serialized).unwrap().unwrap() {
            Packet::MatchID(h) => assert_eq!(h.id, id),
            _ => panic!("Expected Match ID packet"),
        }
        match codec.read(&serialized).unwrap().unwrap() {
            Packet::Handshake(h) => assert_eq!(h.protocol_version, version),
            _ => panic!("Expected Handshake packet"),
        }
    }

    #[test]
    fn packet_decode_none() {
        let mut codec = PacketCodec::default();
        let serialized = vec![0x7F]; // Length indicates 127 bytes, but none are present
        assert!(codec.read(&serialized).unwrap().is_none());
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
        let deserialized = codec.read(&serialized).unwrap().unwrap();

        match deserialized {
            Packet::Handshake(h) => assert_eq!(h.protocol_version, version),
            _ => panic!("Expected Handshake packet"),
        }
    }
}
