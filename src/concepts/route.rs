use std::hash::{Hash, Hasher};
use std::time::Instant;
use crate::concepts::interface::{AddressType};
use crate::concepts::neighbour::Neighbour;
use crate::framework::{RoutingSystem};

pub struct SourceEntry<T: RoutingSystem>{
    pub source: Source<T>,
    pub met_seq: MetSeq
}
pub struct MetSeq{
    /// the metric with which this route was advertised by the neighbour, or FFFF hexadecimal (infinity) for a recently retracted route
    pub metric: u16,
    /// the sequence number with which this route was advertised
    pub seqno: u16,
}

#[derive(Clone)]
pub struct Source<T: RoutingSystem>{
    pub address: T::NodeAddress,
    pub router_id: u64
    // /// (feasibility distance, seqno)
    // pub feasibility: (u16, u16),
    // /// When the garbage-collection timer expires, the entry is removed from the source table.
    // pub timer_last_update: Instant
}

impl<T: RoutingSystem> Hash for Source<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.address.hash(state);
        self.router_id.hash(state);
    }
}

impl<T: RoutingSystem> PartialEq for Source<T> {
    fn eq(&self, other: &Self) -> bool {
        self.address.eq(&other.address) && self.router_id.eq(&other.router_id)
    }
}

impl<T: RoutingSystem> Eq for Source<T> {
}

/// 3.2.6. The Route Table
#[derive(Clone)]
pub struct Route<'owner, T: RoutingSystem> {
    /// the source (address, router-id) for which this route is advertised
    pub source: Source<T>,
    /// the neighbour (an entry in the neighbour table) that advertised this route
    pub neighbour: Option<&'owner Neighbour<'owner, T>>,
    /// the metric with which this route was advertised by the neighbour, or FFFF hexadecimal (infinity) for a recently retracted route
    pub metric: u16,
    /// the sequence number with which this route was advertised
    pub seqno: u16,
    /// the next-hop address of this route
    pub next_hop: T::NodeAddress,
    /// a boolean flag indicating whether this route is selected, i.e., whether it is currently being used for forwarding and is being advertised
    pub selected: bool,
    /// see 3.5.3. Route Acquisition
    pub timer_route_expiry: Instant
}