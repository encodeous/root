use std::time::Instant;
use crate::concepts::interface::{NetworkAddress};
use crate::concepts::neighbour::Neighbour;
use crate::framework::{SystemNetwork, Routing};

pub struct SourceEntry<R: Routing>{
    pub source: Source<R>,
    pub met_seq: MetSeq
}
pub struct MetSeq{
    /// the metric with which this route was advertised by the neighbour, or FFFF hexadecimal (infinity) for a recently retracted route
    pub metric: u16,
    /// the sequence number with which this route was advertised
    pub seqno: u16,
}

pub struct Source<R: Routing>{
    pub address: R::AddressType,
    pub router_id: u64
    // /// (feasibility distance, seqno)
    // pub feasibility: (u16, u16),
    // /// When the garbage-collection timer expires, the entry is removed from the source table.
    // pub timer_last_update: Instant
}

/// 3.2.6. The Route Table
pub struct Route<'owner, R: Routing, N: SystemNetwork> {
    /// the source (address, router-id) for which this route is advertised
    pub source: Source<R>,
    /// the neighbour (an entry in the neighbour table) that advertised this route
    pub neighbour: &'owner Neighbour<'owner, N>,
    /// the metric with which this route was advertised by the neighbour, or FFFF hexadecimal (infinity) for a recently retracted route
    pub metric: u16,
    /// the sequence number with which this route was advertised
    pub seqno: u16,
    /// the next-hop address of this route
    pub next_hop: R::AddressType,
    /// a boolean flag indicating whether this route is selected, i.e., whether it is currently being used for forwarding and is being advertised
    pub selected: bool,
    /// see 3.5.3. Route Acquisition
    pub timer_route_expiry: Instant
}