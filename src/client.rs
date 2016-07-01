use std::sync::{Arc, RwLock};

use packet::{Packet, encode_payload};
use hyper::server::{Handler, Request, Response};
use hyper::method::Method;
use std::ops::Deref;

pub trait Transport {
    fn send(&mut self, Packet);
    fn receive(&mut self) -> Packet;
    fn close(&mut self);
}

pub struct Polling {
    data: Arc<RwLock<Vec<Packet>>>,
    jsonp: Option<i32>,
    b64: bool,
    xhr2: bool,
}

impl Handler for Polling {
    fn handle(&self, req: Request, res: Response) {
        match req.method {
            Method::Get => {
                let d = self.data.clone();
                let mut packets = d.write().unwrap();
                res.send(encode_payload(packets.deref(), self.jsonp, self.b64,
                                        self.xhr2).as_slice());
                packets.clear();
            },
            Method::Post => {
                
            }
            _ => {}
        }
    }
}
