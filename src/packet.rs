use std::str;
use std::vec::IntoIter;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::string::ToString;

use iron::response::Response;
use serialize::base64::{FromBase64, ToBase64, Config, CharacterSet, Newline, FromBase64Error};
use modifier::Modifier;

#[derive(Copy, Clone, Debug, PartialEq)]
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
    pub id: ID,
    pub data: Vec<u8>,
}

#[derive(Debug)]
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

#[inline]
fn u8_to_id(u: u8) -> Result<ID, Error> {
    match u as char {
        '0' => Ok(ID::Open),
        '1' => Ok(ID::Close),
        '2' => Ok(ID::Ping),
        '3' => Ok(ID::Pong),
        '4' => Ok(ID::Message),
        '5' => Ok(ID::Upgrade),
        '6' => Ok(ID::Noop),
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
                        Some(n) => try!(u8_to_id(n)),
                    }
                } else {
                    try!(u8_to_id(n))
                }
            }
        }

        let mut cur = 0;
        let mut data = Vec::with_capacity(data_len - 1);
        while cur < data_len - 1 {
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
        if let Err(_) = str::from_utf8(self.data.as_slice()) {
            true
        } else {
            false
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut vec = Vec::new();

        vec.extend_from_slice((self.id as u8).to_string().as_bytes());
        vec.extend_from_slice(self.data.as_slice());

        vec
    }

    pub fn encode_to(&self, v: &mut Vec<u8>) {
        v.extend_from_slice((self.id as u8).to_string().as_bytes());
        match String::from_utf8(self.data.clone()) {
            Ok(s) => {
                for c in s.chars() {
                    if c == '"' {
                        v.push('\\' as u8);
                        v.push('"' as u8);
                    } else {
                        v.push(c as u8);
                    }
                }
            }
            Err(_) => v.extend_from_slice(self.data.as_slice()),
        };
    }
}

#[derive(Clone)]
pub struct Payload(pub Vec<u8>);

impl Modifier<Response> for Payload {
    fn modify(self, r: &mut Response) {
        r.body = Some(Box::new(self.0));
    }
}

pub fn encode_payload(packets: &Vec<Packet>,
                      jsonp_index: Option<i32>,
                      b64: bool,
                      xhr2: bool)
                      -> Payload {
    let mut data = Vec::new();
    let mut jsonp = false;

    if let Some(index) = jsonp_index {
        data.extend_from_slice(format!("___eio[{}](\"", index).as_bytes());
        jsonp = true;
    }

    for packet in packets {
        let is_binary = packet.is_binary();

        if (b64 || !xhr2) && is_binary {
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
            data.extend_from_slice((packet.id as u8).to_string().as_bytes());
            data.extend_from_slice(base64_data.as_bytes());
        } else {
            if xhr2 {
                data.push(is_binary as u8);
                data.push(0);
                data.push(255);
            }

            data.extend_from_slice((packet.data.len() + 1).to_string().as_bytes());

            data.push(':' as u8);
            packet.encode_to(&mut data);
        }
    }

    if jsonp {
        data.extend_from_slice("\");".as_bytes());
    }

    Payload(data)
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
