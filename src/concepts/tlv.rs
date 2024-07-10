use std::cmp::Ordering;
use std::time::Instant;
use crate::concepts::neighbour::Neighbour;
use crate::framework::{RoutingSystem};

#[derive(Eq, PartialEq, Ord, PartialOrd)]
pub enum Tlv {
    Hello{
        interval_ms: u16
    }
}

pub struct IncomingTlv<'system, T: RoutingSystem> {
    pub tlv: Tlv,
    pub neighbour: &'system Neighbour<'system, T>
}

pub struct OutgoingTlv<'system, T: RoutingSystem> {
    pub send_at: Instant,
    pub tlv: Tlv,
    pub neighbour: &'system Neighbour<'system, T>
}

impl<'system, T: RoutingSystem> Eq for OutgoingTlv<'system, T> {}
impl<'system, T: RoutingSystem> PartialEq<Self> for OutgoingTlv<'system, T> {
    fn eq(&self, other: &Self) -> bool {
        self.send_at == other.send_at &&
            self.tlv == other.tlv &&
            self.neighbour == other.neighbour
    }
}
impl<'system, T: RoutingSystem> PartialOrd<Self> for OutgoingTlv<'system, T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl <'system, T: RoutingSystem> Ord for OutgoingTlv<'system, T>{
    fn cmp(&self, other: &Self) -> Ordering {
        self.send_at.cmp(&other.send_at)
    }
}