use std::time::Instant;
use std::sync::{RwLock, Mutex};
use std::str::FromStr;
use std::borrow::Cow;
use url::Url;
use transport::Transport;
use packet::Packet;

pub struct Socket<C: Transport> {
    sid: String,
    last_pong: Instant,
    //sender: RwLock<Sender>,
    transport: Mutex<C>,
    b64: bool,
    on_close: RwLock<Option<Box<Fn(&str) + 'static>>>,
    on_message: RwLock<Option<Box<Fn(Vec<u8>) + 'static>>>
}

impl<C: Transport> Socket<C> {
    // pub fn new(sid: String, sender: Sender) -> Socket {
    //     Socket{
    //         sid: sid,
    //         last_pong: Instant::now(),
    //         sender: RwLock::new(sender),
    //         b64: false,
    //         on_close: RwLock::new(None),
    //         on_message: RwLock::new(None),
    //     }
    // }

    /// Disconnects the client
    // pub fn close(&mut self) -> Result<()> {
    //     self.sender.write().unwrap().close(CloseCode::Away)
    // }

    pub fn close(&mut self) {
        self.transport.lock().unwrap().close();
    }

    pub fn emit(&mut self, data: Packet) {
        self.transport.lock().unwrap().send(data)
    }

    /// Set callback for when the client is disconnected
    pub fn on_close<F>(&mut self, f: F)
        where F: Fn(&str) + 'static {
        let mut data = self.on_close.write().unwrap();
        if let Some(ref b) = *data {
            drop(b);
        }
        *data = Some(Box::new(f));
    }

    /// Set callback for when client sends a message
    pub fn on_message<F>(&mut self, f: F)
        where F: Fn(Vec<u8>) + 'static {
        let mut data = self.on_message.write().unwrap();
        if let Some(ref b) = *data {
            drop(b);
        }
        *data = Some(Box::new(f));
    }
}


// impl Handler for Socket {
//     fn on_message(&mut self, msg: Message) -> Result<()> {
//         self.on_message.read().unwrap().as_ref().map(|f| f(msg.into_data()));
//         Ok(())
//     }

//     fn on_close(&mut self, code: CloseCode, reason: &str) {
//         self.on_close.read().unwrap().as_ref().map(|f| f(reason));
//     }

//     fn on_request(&mut self, req: &Request) -> Result<Response> {
//         let url = Url::from_str(req.resource()).unwrap();
//         for (key, value) in url.query_pairs() {
//             if key == "transport" && value != "websocket" {
//                 return Err(Error::new(Kind::Internal, "unsupported/unknown transport"))
//             }
//         }

//         Response::from_request(req)
//     }

// }
