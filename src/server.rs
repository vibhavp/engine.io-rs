use std::net::ToSocketAddrs;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use std::str::FromStr;
use std::collections::HashMap;
use std::rc::Rc;

use socket::Socket;
use transport::Transport;
use packet::{Packet, ID};
use url::Url;
use hyper::server::{Handler, Request, Response};
use hyper::method::Method;
use hyper::status::StatusCode;
use hyper::header::{Header, Cookie};

pub struct Server<A: ToSocketAddrs, C: Transport> {
    addr: Arc<A>,
    clients: Arc<RwLock<HashMap<Rc<String>, Socket<C>>>>,
    on_connection: Arc<RwLock<Option<Box<Fn(Socket<C>) + 'static>>>>,
    ping_timeout: Arc<Duration>, // seconds
}

unsafe impl<T: ToSocketAddrs, C: Transport> Send for Server<T, C> {}
unsafe impl<T: ToSocketAddrs, C: Transport> Sync for Server<T, C> {}

const CLOSE: [u8; 5] = [99, 108, 111, 115, 101];
//                     'c'  'l'  'o'  's'  'e'

impl<A: ToSocketAddrs, C: Transport> Server<A, C> {
    pub fn new_with_timeout(addr: A, timeout: Duration) -> Server<A, C> {
        Server {
            addr: Arc::new(addr),
            clients: Arc::new(RwLock::new(HashMap::new())),
            on_connection: Arc::new(RwLock::new(None)),
            ping_timeout: Arc::new(timeout),
        }
    }

    pub fn new(addr: A) -> Server<A, C> {
        Server::new_with_timeout(addr, Duration::from_millis(60000))
    }

    pub fn on_connection<F>(&mut self, f: F)
        where F: Fn(Socket<C>) + 'static
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


    fn open_connection(&self, req: &Request, res: &Response) {}
}

fn get_sid(c: Cookie) -> Option<String> {
    for pair in c.0 {
        if pair.name == "engine-io" {
            return Some(pair.value);
        }
    }

    None
}

impl<A: ToSocketAddrs, C: Transport> Handler for Server<A, C> {
    fn handle(&self, mut req: Request, mut res: Response) {
        let cookie_h = match req.headers.get_raw("Cookie") {
            Some(c) => c,
            None => return,
        };
        let cookie = match Cookie::parse_header(cookie_h) {
            Ok(c) => c,
            _ => {
                self.open_connection(&req, &res);
                return;
            }
        };
        match req.method {
            Method::Get => {}

            _ => {
                *res.status_mut() = StatusCode::MethodNotAllowed;
            }
        }
    }
}
// impl<A: ToSocketAddrs> Factory for Server<A> {
//     type Handler = Socket;
//     fn connection_made(&mut self, s: Sender) -> Socket {
//         Socket::new()
//     }
// }
