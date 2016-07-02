use std::sync::{Arc, Mutex};
use std::io::Read;
use std::ops::Deref;

use packet::{Packet, encode_payload, decode_payload};
use hyper::server::{Handler, Request, Response};
use hyper::method::Method;
use hyper::status::StatusCode;

pub struct Polling {
    send: Arc<Mutex<Vec<Packet>>>,
    recv: Arc<Mutex<Vec<Packet>>>,
    jsonp: Option<i32>,
    b64: bool,
    xhr2: bool,
}

impl Handler for Polling {
    fn handle(&self, mut req: Request, mut res: Response) {
        match req.method {
            Method::Get => {
                let d = self.send.clone();
                let mut packets = d.lock().unwrap();
                res.send(encode_payload(packets.deref(), self.jsonp, self.b64,
                                          self.xhr2).as_slice()).map(|_| packets.clear());
            },
            Method::Post => {
                let mut packets = Vec::new();
                if let Err(_) = req.read_to_end(&mut packets) {
                  return;
                }
                match decode_payload(packets, self.b64, self.xhr2) {
                  Err(e) => {res.send(format!("{}", e).as_bytes());},
                  Ok(mut res) => {
                    let d = self.recv.clone();
                    let mut recv = d.lock().unwrap();
                    recv.append(&mut res);
                  }
              }
            }
            _ => {
              // invalid method
              let mut code = res.status_mut();
              *code = StatusCode::MethodNotAllowed;
          },
        }
    }
}
