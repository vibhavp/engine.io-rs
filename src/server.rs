use std::net::ToSocketAddrs;
use std::sync::RwLock;
use std::time::Duration;
use std::str::FromStr;
use std::sync::Arc;

use socket::Socket;
use client::Transport;
use packet::{Packet, ID};
use url::Url;

pub struct Server<A: ToSocketAddrs, C: Transport> {
    addr: A,
    clients: Arc<RwLock<Vec<Socket<C>>>>,
    on_connection: RwLock<Option<Box<Fn(Socket<C>) + 'static>>>,
    ping_timeout: Duration, //seconds
}

const CLOSE: [u8; 5] = [99, 108, 111, 115, 101];
//                     'c'  'l'  'o'  's'  'e'

impl<A: ToSocketAddrs, C: Transport> Server<A, C> {
    pub fn new_with_timeout(addr: A, timeout: Duration) -> Server<A, C> {
        Server {
            addr: addr,
            clients: Arc::new(RwLock::new(Vec::new())),
            on_connection: RwLock::new(None),
            ping_timeout: timeout,
        }
    }

    pub fn new(addr: A) -> Server<A,C> {
        Server::new_with_timeout(addr, Duration::from_millis(60000))
    }

    pub fn on_connection<F>(&mut self, f: F)
        where F: Fn(Socket<C>) + 'static {
        let mut data = self.on_connection.write().unwrap();
        if let Some(ref b) = *data {
            drop(b);
        }
        *data = Some(Box::new(f));
    }

    pub fn close(&mut self) {
        for socket in self.clients.write().unwrap().iter_mut() {
            socket.close();
        }
    }
}

// impl<A: ToSocketAddrs> Factory for Server<A> {
//     type Handler = Socket;
//     fn connection_made(&mut self, s: Sender) -> Socket {
//         Socket::new()
//     }
// }
