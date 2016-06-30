use std::str;
use std::time::Duration;
use hyper::server::request::Request;
use rand::os::OsRng;
use rand::Rng;

#[derive(Copy, Clone)]
pub enum ID {
    Open = 0,
    Close = 1,
    Ping = 2,
    Pong = 3,
    Message = 4,
    Upgrade = 5,
    Noop = 6,
}

pub struct Packet {
    id: ID,
    data: Vec<u8>,
}

pub enum Error {
    InvalidPacketID(u8),
    Utf8Error(str::Utf8Error),
}

impl Packet {
    pub fn from_bytes(bytes: &[u8]) -> Result<Packet, Error> {
        let id = match bytes[0] {
            0 => ID::Open,
            1 => ID::Close,
            2 => ID::Ping,
            3 => ID::Pong,
            4 => ID::Message,
            5 => ID::Upgrade,
            6 => ID::Noop,
            _ => return Err(Error::InvalidPacketID(bytes[0]))
        };

        let mut data = Vec::with_capacity(bytes.len()-1);
        for b in bytes.iter().skip(1) {
            data.push(*b)
        }

        Ok(Packet{
            id: id,
            data: data
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut vec = Vec::new();

        vec.push(self.id as u8);
        for b in &self.data {
            vec.push(*b)
        }

        vec
    }

    pub fn encode_to(&self, v: &mut Vec<u8>) {
        v.push(self.id as u8);
        for b in &self.data {
            v.push(*b)
        }
    }
    
    fn open_json(sid: String, ping_timeout: Duration) -> String {
        format!("{{\"sid\": {}, \"upgrades\": {}, \"pingTimeout\": {}}}", sid,
                "websocket", ping_timeout.as_secs() * 1000)
    }

    pub fn generate_id(r: &Request) -> String {
        format!("{}{}", r.remote_addr, OsRng::new().unwrap().next_u64())
    }
}

pub fn payload(packets: &[Packet], j: i32, b64: bool) -> Vec<u8> {
    let mut data = Vec::new();
    for packet in packets {
        
    }
}
