use std::time::Instant;
use std::sync::{RwLock, Mutex, Arc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::cell::Cell;
use std::time;

use packet::{Packet, ID, encode_payload, Payload};

#[derive(Clone)]
#[doc(hidden)]
pub enum Transport {
    Polling(Arc<Mutex<Vec<Packet>>>),
}

pub struct Socket {
    transport: Transport,
    sid: Arc<String>,
    last_pong: Arc<RwLock<Instant>>,
    closed: AtomicBool,
    b64: bool,
    xhr2: bool,
    jsonp: Option<i32>,
    on_close: Arc<RwLock<Option<Box<Fn(&str) + 'static>>>>,
    on_message: Arc<RwLock<Option<Box<Fn(&Vec<u8>) + 'static>>>>,
    on_packet: Arc<RwLock<Option<Box<Fn(Packet) + 'static>>>>,
    on_flush: Arc<RwLock<Option<Box<Fn(Vec<Packet>) + 'static>>>>,
}

unsafe impl Send for Socket {}
unsafe impl Sync for Socket {}

impl Socket {
    #[doc(hidden)]
    pub fn new(transport: Transport,
               b64: bool,
               xhr2: bool,
               sid: Arc<String>,
               jsonp: Option<i32>)
               -> Socket {
        Socket {
            transport: transport,
            sid: sid,
            last_pong: Arc::new(RwLock::new(Instant::now())),
            closed: AtomicBool::new(false),
            b64: b64,
            jsonp: jsonp,
            xhr2: xhr2,
            on_close: Arc::new(RwLock::new(None)),
            on_message: Arc::new(RwLock::new(None)),
            on_packet: Arc::new(RwLock::new(None)),
            on_flush: Arc::new(RwLock::new(None)),
        }
    }

    #[doc(hidden)]
    pub fn reset_timeout(&self) {
        *self.last_pong.write().unwrap() = Instant::now();
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
    pub fn close(&self) {
        self.closed.store(false, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }

    pub fn emit(&self, data: Packet) {
        match self.transport {
            Transport::Polling(ref send_buf) => send_buf.lock().unwrap().push(data),
        }
    }

    /// Set callback for when a packet is sent to the client (message, ping)
    pub fn on_packet<F>(&self, f: F)
        where F: Fn(&str) + 'static
    {
        let mut func = self.on_close.write().unwrap();
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
        where F: Fn(&Vec<u8>) + 'static
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
        if let Some(ref func) = *self.on_message.read().unwrap() {
            func(data)
        }
    }

    #[doc(hidden)]
    pub fn call_on_packet(&self, p: Packet) {
        if let Some(ref func) = *self.on_packet.read().unwrap() {
            func(p)
        }
    }

    #[doc(hidden)]
    pub fn encode_write_buffer(&self) -> Payload {
        let Transport::Polling(ref packets) = self.transport;
        let data = packets.clone();
        let vec = data.lock().unwrap();

        encode_payload(&vec, self.jsonp, self.b64, self.xhr2)
    }
    // #[doc(hidden)]
    // pub fn drain_write_buf(&self) -> Vec<Packet> {
    //     let packets = self.
    // }
}
