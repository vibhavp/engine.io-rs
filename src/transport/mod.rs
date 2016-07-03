pub mod polling;

use packet::Packet;

pub trait Transport {
    fn name(&self) -> &'static str;
    fn send(&self, Packet);
    fn receive(&self) -> Option<Packet>;
    fn receive_all(&self) -> Vec<Packet>;
    fn close(&self);
}