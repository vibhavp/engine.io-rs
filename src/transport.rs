use packet::Packet;

pub trait Transport {
    fn name(&self) -> &'static str;
    fn send(&mut self, Packet);
    fn receive(&mut self) -> Packet;
    fn close(&mut self);
}