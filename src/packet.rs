use std::str;
use std::vec::IntoIter;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::string::{FromUtf8Error, ToString};
use std::num::ParseIntError;

use iron::response::Response;
use serialize::base64::{FromBase64, ToBase64, Config, CharacterSet, Newline, FromBase64Error};
use modifier::Modifier;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ID {
    Open = 0,
    Close = 1,
    Ping = 2,
    Pong = 3,
    Message = 4,
    Upgrade = 5,
    Noop = 6,
}

#[derive(Clone, Eq, PartialEq, Debug)]
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
    FromUtf8Error(FromUtf8Error),
    ParseIntError(ParseIntError),
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
             _ => {write!(f, "oops")},
        }
    }
}

impl From<FromBase64Error> for Error {
    fn from(e: FromBase64Error) -> Error {
        Error::FromBase64Error(e)
    }
}

impl From<FromUtf8Error> for Error {
    fn from(e: FromUtf8Error) -> Error {
        Error::FromUtf8Error(e)
    }
}

impl From<ParseIntError> for Error {
    fn from(e: ParseIntError) -> Error {
        Error::ParseIntError(e)
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
    pub fn from_bytes(bytes: &[u8]) -> Result<Packet, Error> {
        if bytes.len() == 0 {
            return Err(Error::EmptyPacket)
        }

        let mut base64 = false;
        let id = if bytes[0] == b'b' {
            base64 = true;
            if bytes.len() < 1 {
                return Err(Error::IncompletePacket)
            }
            try!(u8_to_id(bytes[1]))
        } else {
            try!(u8_to_id(bytes[0]))
        };
        let mut data = Vec::new();

        data.extend(bytes.iter().skip(1));
        Ok(Packet{
            id: id,
            data: if base64 {
                try!(data.from_base64())
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
                char_set: CharacterSet::Standard,
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

    if xhr2 {

    } else {
        let mut data_iter: IntoIter<u8> = data.into_iter();
        while data_iter.by_ref().len() != 0 {
            let len = try!(usize::from_str_radix(&try!(
                String::from_utf8(data_iter.by_ref().
                                  take_while(|c| *c != b':').
                                  collect::<Vec<u8>>())), 10));
            if len > data_iter.by_ref().len() {
                return Err(Error::IncompletePacket);
            }
            packets.push(try!(Packet::from_bytes(&data_iter.by_ref().take(len).
                                                collect::<Vec<u8>>())))
        }
    }

    Ok(packets)
}


#[cfg(test)]
mod tests {
    use super::{decode_payload, ID};
    #[test]
    fn it_works() {
        let packets = decode_payload("6:4Hello11:4HelloWorld".to_string().into_bytes(), true, false).unwrap();
        assert_eq!(packets[0].id, ID::Message);
        assert_eq!(packets[0].data, ("Hello".to_string().into_bytes()));
        assert_eq!(packets[1].id, ID::Message);
        assert_eq!(packets[1].data, ("HelloWorld".to_string().into_bytes()));

        let mut err = decode_payload("asd:asd".to_string().into_bytes(), true, false);
        assert!(err.is_err());
        err = decode_payload("10:2asd".to_string().into_bytes(), true, false);
        assert!(err.is_err());
    }
}
