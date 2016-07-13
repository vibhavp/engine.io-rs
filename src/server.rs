use std::net::ToSocketAddrs;
use std::sync::{Arc, RwLock, Mutex};
use std::borrow::Cow;
use std::time::Duration;
use std::collections::HashMap;
use std::sync::Once;
use std::error::Error as StdError;
use std::str::FromStr;
use std::fmt;

use socket::{Socket, Transport};
use packet::{Packet, ID, decode_payload};
use iron;
use modifier::Modifier;
use iron::method::Method::{Get, Post};
use iron::request::Request;
use iron::response::Response;
use iron::middleware::Handler;
use iron::headers::{Header, Cookie};
use cookie::Cookie as CookiePair;
use iron::IronResult;
use iron::status;
use iron::error::IronError;
use iron::headers::SetCookie;
use url::{Url, form_urlencoded};

pub struct Server {
    clients: Arc<RwLock<HashMap<Arc<String>, Arc<Socket>>>>,
    on_connection: Arc<RwLock<Option<Box<Fn(Arc<Socket>) + 'static>>>>,
    ping_timeout: Duration, // seconds
    cookie_path: Option<String>,
}

#[derive(Debug, Copy, Clone)]
enum Error {
    UnsupportedTransport,
    InvalidSID,        
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", format!("{}", self.description()))
    }
}

impl StdError for Error {
    fn description(&self) -> &'static str {
        match self{
            &Error::UnsupportedTransport => "unsupported/invalid transport",
            &Error::InvalidSID => "invalid session ID",                
        }
    }
}

impl Modifier<Response> for Error {
    fn modify(self, res: &mut Response) {
        res.body = Some(Box::new(String::from_str(self.description()).unwrap()))
    }
}

unsafe impl Send for Server {}
unsafe impl Sync for Server {}

macro_rules! make_err {
    ($x:expr) => {Err(IronError::new(Box::new($x), $x));}
}

impl Server {
    pub fn new(timeout: Duration, cookie_path: Option<String>) -> Server {
        Server {
            clients: Arc::new(RwLock::new(HashMap::new())),
            on_connection: Arc::new(RwLock::new(None)),
            ping_timeout: timeout,
            cookie_path: cookie_path,
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
        let url: Url = req.url.clone().into_generic_url();
        let pairs = url.query_pairs().into_owned();
        let map = into_hashmap(pairs);

        // transport: indicates the transport name. Supported ones by
        // default are polling, flashsocket, websocket.
        let transport = match map.get("transport") {
            Some(s) if s == "polling" => Transport::Polling(Arc::new(Mutex::new(Vec::new()))),
            //add websocket later
            _ => {
                return make_err!(Error::UnsupportedTransport);
            },
        };

        // j: is the transport is polling but a JSONP respose is required, j
        // must be set with the JSONP index.
        let jsonp: Option<_> = match map.get("jsonp") {
            Some(j) => Some(itry!(i32::from_str_radix(j, 10), status::BadRequest)),
            None => None,
        };

        // b64: if the client doesn't support XHR2, b64=1 is sent in the query
        // string to signal the server that all binary data should be sent base64
        // encoded
        let b64 = map.get("b64").is_some();

        let sid = match map.get("sid") {
            Some(s) => {
                if let Some(so) = self.clients.read().unwrap().get(s) {
                    so.reset_timeout();
                    let mut res = Response::new();
                    let cookie = CookiePair::new(String::from_str("io").unwrap(), s.clone());
                    //TODO: Set cookie path
                    res.headers.set(SetCookie(vec![cookie]));
                    return Ok(res);
                }
                return make_err!(Error::InvalidSID);
            },
            None => Arc::new(Packet::generate_id(req)) 
        };

        
        let so = Socket::new(sid.clone(), transport, b64,jsonp);
        self.clients.write().unwrap().insert(sid, Arc::new(so));

        Ok(Response::new())
    }
}

fn into_hashmap(pairs: form_urlencoded::ParseIntoOwned) -> HashMap<String, String> {
    let mut h = HashMap::new();
    for (q, v) in pairs {
        h.insert(q, v);
    }

    h
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
            },
            Get => {
                let payload = so.encode_write_buffer();
                let res = Response::with(payload);
                Ok(res)
            },
            _ => {
                let mut res = Response::new();
                res.status = Some(status::MethodNotAllowed);
                Ok(res)
            }
        }
    }
}
