use std::str;
use std::time::Duration;
use hyper::server::request::Request;
use rand::os::OsRng;
use rand::Rng;
use rustc_serialize::base64::{ToBase64, Config, CharacterSet, Newline};

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

    fn is_binary(&self) -> bool {
        for c in &self.data {
            if *c < 'a' as u8 || *c > 'Z' as u8 {
                return true
            }
        }
        false
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

/// Encode a series of packets, for polling and flashsocket transports.
///
/// # Arguments:
///
/// * `packets`: A vector of packets to be encoded together.
/// * `jsonp_index`: JSONP response index, if any.
/// * `b64`: true if binary packets are to be encoded in base64.
/// * `xhr2`: true if the packets are to be encoded with XHR2 support.
pub fn encode_payload(packets: Vec<Packet>, jsonp_index: Option<i32>, b64: bool, xhr2: bool) -> Vec<u8> {
    let mut data = Vec::new();
    let mut jsonp = false;

    jsonp_index.map(|index| {
        for c in format!("__eio[{}](",index).as_bytes() {
            data.push(*c);
        }
        jsonp = true;
    });

    for packet in packets {
        if b64 {
            let base64_data = packet.data.to_base64(Config{
                char_set: CharacterSet::UrlSafe,
                newline: Newline::LF,
                pad: true,
                line_length: None,
            });
            for c in (base64_data.len() + 1).to_string().chars() {
                data.push(c.to_digit(10).unwrap() as u8);
            }
            data.push(':' as u8);
            data.push('b' as u8);
            data.extend_from_slice(base64_data.as_bytes());
        } else {
            if xhr2 {
                data.push(packet.is_binary() as u8);
                data.push(0);
                data.push(255);
            }

            for c in packet.data.len().to_string().chars() {
                data.push(c.to_digit(10).unwrap() as u8);
            }
            data.push(':' as u8);
            data.extend_from_slice(packet.encode().as_slice());
        }
    }

    if jsonp {
        data.push(')' as u8);
    }

    data
}
