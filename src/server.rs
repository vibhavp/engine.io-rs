use std::net::ToSocketAddrs;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use std::collections::HashMap;
use std::sync::Once;

use socket::Socket;
use packet::{Packet, ID, decode_payload};
use iron;
use iron::method::Method::{Get, Post};
use iron::request::Request;
use iron::response::Response;
use iron::middleware::Handler;
use iron::headers::{Header, Cookie};
use iron::IronResult;
use iron::status;
use iron::error::IronError;
use url::Url;

pub struct Server {
    clients: Arc<RwLock<HashMap<Arc<String>, Arc<Socket>>>>,
    on_connection: Arc<RwLock<Option<Box<Fn(Arc<Socket>) + 'static>>>>,
    ping_timeout: Duration, // seconds
}

unsafe impl Send for Server {}
unsafe impl Sync for Server {}

impl Server {
    pub fn new(timeout: Duration) -> Server {
        Server {
            clients: Arc::new(RwLock::new(HashMap::new())),
            on_connection: Arc::new(RwLock::new(None)),
            ping_timeout: timeout,
        }
    }

    pub fn on_connection<F>(&mut self, f: F)
        where F: Fn(Arc<Socket>) + 'static
    {
        let mut data = self.on_connection.write().unwrap();
        if let Some(ref b) = *data {
            drop(b);
        }
        *data = Some(Box::new(f));
    }

    pub fn close(&mut self) {
        for (_, socket) in self.clients.write().unwrap().iter_mut() {
            socket.close();
        }
    }

    fn get_sid(c: Cookie) -> Option<String> {
        for pair in c.0 {
            if pair.name == "engine-io" {
                return Some(pair.value);
            }
        }

        None
    }

    pub fn get_socket(&self, c: Cookie) -> Option<Arc<Socket>> {
        for pair in c.0 {
            if pair.name == "engine-io" {
                let map = self.clients.read().unwrap();
                return match map.get(&pair.value) {
                    Some(so) => Some(so.clone()),
                    None => None,
                };
            }

        }
        None
    }

    pub fn remove_socket(&self, sid: String) {
        let mut map = self.clients.write().unwrap();
        map.remove(&sid);
    }

    fn open_connection(&self, req: &Request) -> IronResult<Response> {
        Ok(Response::new())
    }
}


impl Handler for Server {
    fn handle(&self, req: &mut Request) -> IronResult<Response> {
        let cookies_raw = match req.headers.get_raw("Cookie") {
            Some(c) => c,
            None => return self.open_connection(req),
        };
        let so: Arc<Socket> = match self.get_socket(itry!(Cookie::parse_header(cookies_raw))) {
            Some(so) => so,
            None => return self.open_connection(req),
        };

        match req.method {
            Post => {
                let url: Url = req.url.clone().into_generic_url();
                let query = url.query_pairs().into_owned();
                for (q, val) in query {
                    if q == "d" {
                        // POSTing data
                        val.replace("\\n", "\n");
                        match decode_payload(val.into_bytes(), so.b64(), so.xhr2()) {
                            Ok(packets) => {
                                for packet in packets {
                                    match packet.id {
                                        ID::Close => so.close(),
                                        ID::Pong => so.reset_timeout(),
                                        ID::Message => so.call_on_message(&packet.data),
                                        _ => {
                                            // handle upgrade}
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                let mut res = Response::new();
                                res.status = Some(status::BadRequest);
                                return Ok(res);
                            }
                        }
                    }
                }

                let mut res = Response::new();
                res.status = Some(status::BadRequest);
                Ok(res)
            }
            Get => {
                let payload = so.encode_write_buffer();
                let res = Response::with(payload);
                Ok(res)
            }
            _ => {
                let mut res = Response::new();
                res.status = Some(status::MethodNotAllowed);
                Ok(res)
            }
        }
    }
}
