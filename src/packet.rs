use std::str;
use std::vec::IntoIter;
use std::time::Duration;
use std::fmt;
use std::fmt::{Display, Debug, Formatter};

use hyper::server::request::Request;
use rand::os::OsRng;
use rand::Rng;
use serialize::base64::{FromBase64, ToBase64, Config, CharacterSet, Newline, FromBase64Error};
use crypto::sha2::Sha256;
use crypto::digest::Digest;

#[derive(Copy, Clone, Debug)]
pub enum ID {
    Open = 0,
    Close = 1,
    Ping = 2,
    Pong = 3,
    Message = 4,
    Upgrade = 5,
    Noop = 6,
}

#[derive(Clone)]
pub struct Packet {
    id: ID,
    data: Vec<u8>,
}

pub enum Error {
    InvalidPacketID(u8),
    InvalidLengthDigit(u32),
    InvalidLengthCharacter(u8),
    IncompletePacket,
    EmptyPacket,
    FromBase64Error(FromBase64Error),
    Utf8Error(str::Utf8Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            &Error::InvalidPacketID(id) => write!(f, "Invalid Packet ID: {}", id),
            &Error::InvalidLengthDigit(d) => write!(f, "Invalid length digit: {}", d),
            &Error::InvalidLengthCharacter(d) => write!(f, "Invalid length character: {}", d),
            &Error::IncompletePacket => write!(f, "Incomplete Packet"),
            &Error::EmptyPacket => write!(f, "Empty Packet"),
            &Error::FromBase64Error(e) => write!(f, "FromBase64Error: {}", e),
            &Error::Utf8Error(e) => write!(f, "Utf8Error: {}", e),
            // _ => {write!(f, "oops")},
        }
    }
}

fn u8_to_ID(u: u8) -> Result<ID, Error> {
    match u {
        0 => Ok(ID::Open),
        1 => Ok(ID::Close),
        2 => Ok(ID::Ping),
        3 => Ok(ID::Pong),
        4 => Ok(ID::Message),
        5 => Ok(ID::Upgrade),
        6 => Ok(ID::Noop),
        _ => Err(Error::InvalidPacketID(u)),
    }
}

impl Packet {
    fn from_bytes(bytes: &mut IntoIter<u8>, data_len: usize) -> Result<Packet, Error> {
        let id;
        let mut base64 = false;
        match bytes.next() {
            None => return Err(Error::IncompletePacket),
            Some(n) => {
                id = if n as char == 'b' {
                    // base64
                    base64 = true;
                    match bytes.next() {
                        None => return Err(Error::IncompletePacket),
                        Some(n) => try!(u8_to_ID(n)),
                    }
                } else {
                    try!(u8_to_ID(n))
                }
            }
        }

        let mut cur = 0;
        let mut data = Vec::with_capacity(bytes.len() - data_len - 2);

        while cur < data_len {
            data.push(bytes.next().unwrap());
            cur += 1;
        }

        Ok(Packet {
            id: id,
            data: if base64 {
                try!(data.from_base64().map_err(|e| Error::FromBase64Error(e)))
            } else {
                data
            },
        })
    }

    fn is_binary(&self) -> bool {
        for c in &self.data {
            if *c < 'a' as u8 || *c > 'Z' as u8 {
                return true;
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
        Packet {
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
        hasher.input_str(format!("{}{}", r.remote_addr, OsRng::new().unwrap().next_u32()).as_str());
        hasher.result_str()
    }
}

pub fn encode_payload(packets: &Vec<Packet>,
                      jsonp_index: Option<i32>,
                      b64: bool,
                      xhr2: bool)
                      -> Vec<u8> {
    let mut data = Vec::new();
    let mut jsonp = false;

    if let Some(index) = jsonp_index {
        data.extend_from_slice(format!("__eio[{}](", index).as_bytes());
        jsonp = true;
    }

    for packet in packets {
        if b64 || (!xhr2 && packet.is_binary()) {
            let base64_data = packet.data.to_base64(Config {
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

            for c in (packet.data.len() + 1).to_string().chars() {
                data.push(c.to_digit(10).unwrap() as u8);
            }
            data.push(':' as u8);
            packet.encode_to(&mut data);
        }
    }

    if jsonp {
        data.push(')' as u8);
    }

    data
}

pub fn decode_payload(data: Vec<u8>, b64: bool, xhr2: bool) -> Result<Vec<Packet>, Error> {
    if data.len() == 0 {
        return Err(Error::EmptyPacket);
    }

    let mut packets = Vec::new();
    let mut parsing_length = true;

    if xhr2 {

    } else {
        let mut len: usize = 0;
        let mut data_iter: IntoIter<u8> = data.into_iter();
        while let Some(c) = data_iter.next() {
            if c as char == ':' {
                parsing_length = false;
                // Check for incomplete payload
                if data_iter.len() < len {
                    return Err(Error::IncompletePacket);
                }

                packets.push(try!(Packet::from_bytes(&mut data_iter, len)));
            } else {
                parsing_length = true;
                if let Some(n) = (c as char).to_digit(10) {
                    if n > 9 {
                        return Err(Error::InvalidLengthDigit(n));
                    };
                    len = (len * 10) + n as usize;
                } else {
                    // Invalid length character
                    return Err(Error::InvalidLengthCharacter(c));
                }
            }
        }
    }

    if parsing_length {
        Err(Error::IncompletePacket)
    } else {
        Ok(packets)
    }
}
