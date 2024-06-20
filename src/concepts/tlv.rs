use std::cmp::Ordering;
use std::time::Instant;
use crate::concepts::neighbour::Neighbour;
use crate::framework::SystemNetwork;

#[derive(Eq, PartialEq, Ord, PartialOrd)]
pub enum Tlv {
    Hello{
        interval_ms: u16
    }
}

pub struct IncomingTlv<'system, N: SystemNetwork> {
    pub tlv: Tlv,
    pub neighbour: &'system Neighbour<'system, N>
}

pub struct OutgoingTlv<'system, N: SystemNetwork> {
    pub send_at: Instant,
    pub tlv: Tlv,
    pub neighbour: &'system Neighbour<'system, N>
}

impl<'system, N: SystemNetwork> Eq for OutgoingTlv<'system, N> {}
impl<'system, N: SystemNetwork> PartialEq<Self> for OutgoingTlv<'system, N> {
    fn eq(&self, other: &Self) -> bool {
        self.send_at == other.send_at &&
            self.tlv == other.tlv &&
            self.neighbour == other.neighbour
    }
}
impl<'system, N: SystemNetwork> PartialOrd<Self> for OutgoingTlv<'system, N> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl <'system, N: SystemNetwork> Ord for OutgoingTlv<'system, N>{
    fn cmp(&self, other: &Self) -> Ordering {
        self.send_at.cmp(&other.send_at)
    }
}