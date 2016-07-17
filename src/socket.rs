use std::time::Instant;
use std::sync::{RwLock, Mutex, Arc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::mpsc::{Sender, Receiver};

use packet::{Packet, encode_payload, Payload};

#[derive(Clone)]
#[doc(hidden)]
pub enum Transport {
    Polling(Sender<Packet>, Arc<Mutex<Receiver<Packet>>>),
}

pub struct Socket {
    transport: Transport,
    sid: Arc<String>,
    last_pong: Arc<RwLock<Instant>>,
    last_ping: Arc<RwLock<Instant>>,
    closed: AtomicBool,
    b64: bool,
    xhr2: bool,
    jsonp: Option<i32>,
    client_map: Arc<RwLock<HashMap<Arc<String>, Arc<Socket>>>>,
    on_close: Arc<RwLock<Option<Box<Fn(&str) + 'static>>>>,
    on_message: Arc<RwLock<Option<Box<Fn(Vec<u8>) + 'static>>>>,
    on_packet: Arc<RwLock<Option<Box<Fn(Packet) + 'static>>>>,
    on_flush: Arc<RwLock<Option<Box<Fn(Vec<Packet>) + 'static>>>>,
}

unsafe impl Send for Socket {}
unsafe impl Sync for Socket {}

impl Socket {
    #[doc(hidden)]
    pub fn new(sid: Arc<String>,
               transport: Transport,
               client_map: Arc<RwLock<HashMap<Arc<String>, Arc<Socket>>>>,
               b64: bool,
               jsonp: Option<i32>)
               -> Socket {
        Socket {
            transport: transport,
            sid: sid,
            last_pong: Arc::new(RwLock::new(Instant::now())),
            last_ping: Arc::new(RwLock::new(Instant::now())),
            closed: AtomicBool::new(false),
            b64: b64,
            jsonp: jsonp,
            xhr2: !b64,
            client_map: client_map,
            on_close: Arc::new(RwLock::new(None)),
            on_message: Arc::new(RwLock::new(None)),
            on_packet: Arc::new(RwLock::new(None)),
            on_flush: Arc::new(RwLock::new(None)),
        }
    }

    pub fn id(&self) -> String {
        String::from_str(self.sid.clone().as_str()).unwrap()
    }

    #[doc(hidden)]
    pub fn reset_timeout(&self) {
        *self.last_pong.write().unwrap() = Instant::now();
    }

    #[doc(hidden)]
    pub fn reset_last_ping(&self) {
        let data = self.last_ping.clone();
        let mut instant = data.write().unwrap();
        *instant = Instant::now();
    }

    pub fn get_last_pong(&self) -> Instant {
        let data = self.last_pong.clone();
        let instant = data.read().unwrap();
        *instant
    }

    pub fn get_last_ping(&self) -> Instant {
        let data = self.last_pong.clone();
        let instant = data.read().unwrap();
        *instant
    }

    #[inline(always)]
    pub fn b64(&self) -> bool {
        self.b64
    }

    #[inline(always)]
    pub fn xhr2(&self) -> bool {
        self.xhr2
    }

    #[inline(always)]
    pub fn jsonp_index(&self) -> Option<i32> {
        self.jsonp
    }

    #[inline(always)]
    pub fn close(&self, reason: &str) {
        self.closed.store(true, Ordering::Relaxed);
        let data = self.client_map.clone();
        let mut map = data.write().unwrap();
        map.remove(&self.sid);
        self.on_close.read().unwrap().as_ref().map(|f| f(reason));
    }

    #[inline(always)]
    pub fn closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }

    pub fn emit(&self, data: Packet) {
        if self.closed.load(Ordering::Relaxed) {
            return;
        }
        debug!("sending ID {:?}", data.id);
        match self.transport {
            Transport::Polling(ref send, _) => send.send(data).unwrap(),
        }
    }

    /// Set callback for when a packet is sent to the client (message, ping)
    pub fn on_packet<F>(&self, f: F)
        where F: Fn(Packet) + 'static
    {
        let mut func = self.on_packet.write().unwrap();
        if let Some(ref b) = *func {
            drop(b)
        }
        *func = Some(Box::new(f));
    }

    /// Set callback for when the write buffer is flushed
    pub fn on_flush<F>(&self, f: F)
        where F: Fn(Vec<Packet>) + 'static
    {
        let mut func = self.on_flush.write().unwrap();
        if let Some(ref b) = *func {
            drop(b)
        }
        *func = Some(Box::new(f))
    }

    /// Set callback for when the client is disconnected
    pub fn on_close<F>(&self, f: F)
        where F: Fn(&str) + 'static
    {
        let mut data = self.on_close.write().unwrap();
        if let Some(ref b) = *data {
            drop(b);
        }
        *data = Some(Box::new(f));
    }

    /// Set callback for when client sends a message
    pub fn on_message<F>(&self, f: F)
        where F: Fn(Vec<u8>) + 'static
    {
        let mut data = self.on_message.write().unwrap();
        if let Some(ref b) = *data {
            drop(b);
        }
        *data = Some(Box::new(f));
    }

    #[inline]
    #[doc(hidden)]
    pub fn call_on_message(&self, data: &Vec<u8>) {
        if self.closed() {
            return;
        }
        if let Some(ref func) = *self.on_message.read().unwrap() {
            func(data.clone())
        }
    }

    #[doc(hidden)]
    pub fn call_on_packet(&self, p: Packet) {
        if self.closed() {
            return;
        }
        if let Some(ref func) = *self.on_packet.read().unwrap() {
            func(p)
        }
    }

    #[doc(hidden)]
    pub fn encode_write_buffer(&self) -> Payload {
        let Transport::Polling(_, ref lock) = self.transport;
        let mut packets = vec![];
        let recv = lock.lock().unwrap();

        packets.push(recv.recv().unwrap());
        while let Ok(packet) = recv.try_recv() {
            packets.push(packet)
        }

        let payload = encode_payload(&packets, self.jsonp, self.b64, self.xhr2);
        self.call_on_flush(packets);
        payload
    }

    #[inline]
    fn call_on_flush(&self, packets: Vec<Packet>) {
        if self.closed() {
            return;
        }

        if let Some(ref func) = *self.on_flush.read().unwrap() {
            func(packets)
        }
    }
}
