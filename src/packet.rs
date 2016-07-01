use std::str;
use std::vec::IntoIter;
use std::time::Duration;

use hyper::server::request::Request;
use rand::os::OsRng;
use rand::Rng;
use serialize::base64::{ToBase64, Config, CharacterSet, Newline};
use crypto::sha2::Sha256;
use crypto::digest::Digest;

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
    IncompletePacket,
    Utf8Error(str::Utf8Error),
}

impl Packet {
    fn from_bytes(bytes: &mut IntoIter<u8>, data_len: usize) -> Result<Packet, Error> {
        let id;
        match bytes.next() {
            None => return Err(Error::IncompletePacket),
            Some(n) => {
               id = match n {
                   0 => ID::Open,
                   1 => ID::Close,
                   2 => ID::Ping,
                   3 => ID::Pong,
                   4 => ID::Message,
                   5 => ID::Upgrade,
                   6 => ID::Noop,
                   _ => return Err(Error::InvalidPacketID(n))
               };
            }
        }

        let mut cur = 0;
        let mut data = Vec::with_capacity(bytes.len()-data_len-2);

        while cur < data_len {
            data.push(bytes.next().unwrap());
            cur +=1;
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
        vec.extend_from_slice(self.data.as_slice());

        vec
    }

    pub fn encode_to(&self, v: &mut Vec<u8>) {
        v.push(self.id as u8);
        v.extend_from_slice(self.data.as_slice());
    }

    fn open_json(sid: String, ping_timeout: Duration) -> Packet {
        Packet{
            id: ID::Open,
            data: format!("{{\"sid\": {}, \"upgrades\": {}, \"pingTimeout\": {}}}",
                          sid,
                          "websocket",
                          ping_timeout.as_secs() * 1000)
                .into_bytes(),
        }
    }

    pub fn generate_id(r: &Request) -> String {
        let mut hasher = Sha256::new();
        hasher.input_str(format!("{}{}", r.remote_addr,
                                 OsRng::new().unwrap().next_u32()).as_str());
        hasher.result_str()
    }
}

pub fn encode_payload(packets: &Vec<Packet>, jsonp_index: Option<i32>, b64: bool, xhr2: bool) -> Vec<u8> {
    let mut data = Vec::new();
    let mut jsonp = false;

    jsonp_index.map(|index| {
        data.extend_from_slice(format!("__eio[{}](",index).as_bytes());
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
            data.push(packet.id as u8);
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

pub fn decode_payload(data: Vec<u8>, b64: bool, xhr2: bool)
                      -> Option<Vec<Packet>> {
    if data.len() == 0 {
        return None;
    }

    let mut packets = Vec::new();
    let mut parsing_length = true;

    if xhr2 {

    } else if b64 {

    } else {
        let mut len: usize = 0;
        let mut data_iter: IntoIter<u8> = data.into_iter();
        while let Some(c) = data_iter.next() {
            if c as char == ':' {
                parsing_length = false;
                //Check for incomplete payload
                if data_iter.len() < len {return None}
                if let Ok(packet) = Packet::from_bytes(&mut data_iter, len) {
                    packets.push(packet);
                } else {return None}
            } else {
                parsing_length = true;
                if let Some(n) = (c as char).to_digit(10) {
                    if n > 9 {return None};
                    len = (len*10) + n as usize;
                } else {
                    //Invalid length character
                    return None
                }
            }
        }
    }

    if parsing_length {
        None
    } else {
        Some(packets)
    }
}
