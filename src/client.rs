use std::sync::{Arc, RwLock};

use packet::Packet;
use hyper::server::{Handler, Request, Response};
use hyper::method::Method;

pub trait Client {
    fn send(&mut self, Packet);
    fn receive(&mut self) -> Packet;
    fn close(&mut self);
}

pub struct Polling {
    data: Arc<RwLock<Vec<Packet>>>,
    jsonp: bool,
    b64: bool,
}

impl Handler for Polling {
    fn handle(&self, req: Request, res: Response) {
        match req.method {
            Method::Get => {
                let queue = self.data.clone();
                let mut data = Vec::new();
                for packet in queue.read().unwrap().iter() {
                    packet.encode_to(&mut data);
                }
            },
            _ => {}
        }
    }
}
