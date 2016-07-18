use std::sync::{Arc, RwLock, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::time::Duration;
use std::collections::HashMap;
use std::error::Error as StdError;
use std::str::FromStr;
use std::fmt;
use std::thread::{sleep, spawn};
use std::io::Read;

use socket::{Socket, Transport};
use packet::{Packet, ID, decode_payload, encode_payload};
use config::Config;
use modifier::Modifier;
use iron::method::Method::{Get, Post};
use iron::request::Request;
use iron::response::Response;
use iron::middleware::Handler;
use iron::headers::{Header, Cookie, ConnectionOption, Connection, Date, HttpDate, ContentType};
use hyper::mime::Mime;
use cookie::Cookie as CookiePair;
use url::form_urlencoded::parse;
use iron::IronResult;
use iron::status;
use iron::error::IronError;
use iron::headers::SetCookie;
use url::{Url, form_urlencoded};
use time;

#[derive(Clone)]
pub struct Server {
    clients: Arc<RwLock<HashMap<Arc<String>, Socket>>>,
    on_connection: Arc<RwLock<Option<Box<Fn(Socket) + 'static>>>>,
    ping_loop_started: Arc<AtomicBool>,
    config: Arc<Config>,
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
        match self {
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
    pub fn new() -> Server {
        Server::with_config(Default::default())
    }

    pub fn with_config(config: Config) -> Server {
        Server {
            clients: Arc::new(RwLock::new(HashMap::new())),
            on_connection: Arc::new(RwLock::new(None)),
            ping_loop_started: Arc::new(AtomicBool::new(false)),
            config: Arc::new(config),
        }
    }

    pub fn on_connection<F>(&self, f: F)
        where F: Fn(Socket) + 'static
    {
        let mut data = self.on_connection.write().unwrap();
        if let Some(ref b) = *data {
            drop(b);
        }
        *data = Some(Box::new(f));
    }

    pub fn close(&self) {
        let data = self.clients.clone();
        let mut map = data.write().unwrap();

        for (_, socket) in map.iter_mut() {
            socket.close("closing server");
        }

        map.clear();
    }

    pub fn get_socket(&self, c: Cookie) -> Option<Socket> {
        for pair in c.0 {
            if pair.name == "io" {
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
        debug!("opening new connection to {}", req.remote_addr);
        let url: Url = req.url.clone().into_generic_url();
        let pairs = url.query_pairs().into_owned();
        let map = into_hashmap(pairs);

        // transport: indicates the transport name. Supported ones by
        // default are polling, flashsocket, websocket.
        let transport = match map.get("transport") {
            Some(s) if s == "polling" => {
                let (send, recv) = channel();
                Transport::Polling(send, Arc::new(Mutex::new(recv)))
            }
            // add websocket later
            _ => {
                return make_err!(Error::UnsupportedTransport);
            }
        };

        // j: is the transport is polling but a JSONP respose is required, j
        // must be set with the JSONP index.
        let jsonp: Option<_> = match map.get("j") {
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
                    // TODO: Set cookie path
                    res.headers.set(SetCookie(vec![cookie]));
                    return Ok(res);
                }
                return make_err!(Error::InvalidSID);
            }
            None => Arc::new((*self.config.generate_id)(req)),
        };


        let so = Socket::new(sid.clone(), transport, self.clients.clone(), b64, jsonp);
        self.clients.write().unwrap().insert(sid.clone(), so.clone());
        // if transport is not polling
        // so.emit(open_json(sid.clone(), Duration::from_secs(2)));

        self.on_connection.read().unwrap().as_ref().map(|func| func(so.clone()));

        let mut res = Response::new();
        res.status = Some(status::Ok);
        let cookie = CookiePair::new(String::from_str("io").unwrap(), so.id());
        // TODO: Set cookie path
        res.headers.set(SetCookie(vec![cookie]));
        res.headers.set(Connection(vec![ConnectionOption::KeepAlive]));
        res.headers.set(Date(HttpDate(time::now())));
        let mime: Mime = "text/javascript".parse().unwrap();
        res.headers.set(ContentType(mime));

        // if transport is polling
        res.body = Some(Box::new(encode_payload(&vec![self.open_json(sid.clone())],
                                                so.jsonp_index(),
                                                so.b64(),
                                                so.xhr2())
            .0));
        Ok(res)
    }

    fn ping_loop(&self) {
        let data = self.clients.clone();

        loop {
            {
                let mut map = data.write().unwrap();
                let timedout_clients: Vec<Arc<String>> = map.iter()
                    .filter_map(|(sid, so)| {
                        let instant = so.get_last_pong();
                        if instant.elapsed().as_secs() * 1000 > 65000 {
                            // no pong response for < 60000 seconds
                            Some(sid.clone())
                        } else {
                            None
                        }
                    })
                    .collect();

                for sid in timedout_clients {
                    map.remove(&sid);
                }
            }

            {
                let map = data.read().unwrap();
                for (_, so) in map.iter() {
                    let instant = so.get_last_ping();
                    if instant.elapsed().as_secs() * 1000 > 20000 {
                        so.emit(Packet {
                            id: ID::Ping,
                            data: (b"ping").to_vec(),
                        });
                        so.reset_last_ping();
                    }
                }
            }

            sleep(Duration::from_secs(1));
        }
    }

    fn open_json(&self, sid: Arc<String>) -> Packet {
        let s = format!(r#"{{"sid":"{}","upgrades":[],"pingTimeout":{},"pingInterval":{}}}"#,
                        sid,
                        self.config.ping_timeout.as_secs() * 1000,
                        self.config.ping_interval.as_secs() * 1000);
        Packet {
            id: ID::Open,
            data: s.into_bytes(),
        }
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
        if !self.ping_loop_started.clone().compare_and_swap(false, true, Ordering::SeqCst) {
            let cl = self.clone();
            spawn(move || cl.ping_loop());
        }

        let cookies_raw = match req.headers.get_raw("Cookie") {
            Some(c) => c,
            None => return self.open_connection(req),
        };
        let so: Socket = match self.get_socket(itry!(Cookie::parse_header(cookies_raw))) {
            Some(so) => so,
            None => return self.open_connection(req),
        };

        let mut res = Response::new();
        res.headers.set(Date(HttpDate(time::now())));

        match req.method {
            Post => {
                let mut body = Vec::new();
                itry!(req.body.read_to_end(&mut body));
                let form = parse(body.as_slice());
                let mut closing = false;

                for (q, val) in form {
                    if q == "d" {
                        // POSTing data
                        val.replace("\\n", "\n");
                        match decode_payload(val.into_owned().into_bytes(), so.b64(), so.xhr2()) {
                            Ok(packets) => {
                                for packet in packets {
                                    match packet.id {
                                        ID::Close => {
                                            so.close("close requested by client");
                                            closing = true;
                                        }
                                        ID::Pong if packet.data.as_slice() == b"ping" => {
                                            so.reset_timeout()
                                        }

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

                if !closing {
                    res.headers.set(Connection(vec![ConnectionOption::KeepAlive]));
                }

                let mime: Mime = "text/html".parse().unwrap();
                res.headers.set(ContentType(mime));
                res.body = Some(Box::new("ok"));
                res.status = Some(status::Ok);

                Ok(res)
            }
            Get => {
                let payload = so.encode_write_buffer();
                let mut res = Response::with(payload);
                res.status = Some(status::Ok);
                if so.jsonp_index().is_some() {
                    let mime: Mime = "text/javascript".parse().unwrap();
                    res.headers.set(ContentType(mime));
                }
                res.headers.set(Connection(vec![ConnectionOption::KeepAlive]));
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
