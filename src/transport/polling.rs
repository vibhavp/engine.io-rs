use std::sync::{Arc, Mutex, Condvar};
use std::io::Read;
use std::ops::Deref;

use packet::{Packet, encode_payload, decode_payload};
use transport::Transport;
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
            },
            _ => {
              // invalid method
              let mut code = res.status_mut();
              *code = StatusCode::MethodNotAllowed;
          },
        }
    }
}

impl Transport for Polling {
  fn name(&self) -> &'static str {
    "polling"
  }

  fn send(&self, packet: Packet) {
    let d = self.send.clone();
    let mut send = d.lock().unwrap();
    send.push(packet);
  }

  fn receive(&self) -> Option<Packet> {
    let d = self.recv.clone();
    let mut recv = d.lock().unwrap();
    recv.pop()
  }

  fn receive_all(&self) -> Vec<Packet> {
    let d = self.recv.clone();
    let mut recv = d.lock().unwrap();
    recv.clone()
  }

  fn close(&self) {

  }
}