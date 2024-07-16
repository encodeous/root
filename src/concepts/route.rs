use std::hash::{Hash, Hasher};
use std::time::Instant;
use crate::concepts::interface::{AddressType, Interface};
use crate::concepts::neighbour::Neighbour;
use crate::framework::{RoutingSystem};

/// 3.2.6. The Route Table
#[derive(Clone)]
pub struct Route<'owner, T: RoutingSystem> {
    /// the source for which this route is advertised
    pub source: T::NodeAddress,
    /// the neighbour (an entry in the neighbour table) that advertised this route
    pub neighbour: Option<&'owner Neighbour<'owner, T>>,
    /// the interface that the neighbour is available on
    pub itf: Option<T::InterfaceId>,
    /// the metric with which this route was advertised by the neighbour, or FFFF hexadecimal (infinity) for a recently retracted route
    pub metric: u16,
    /// the sequence number with which this route was advertised
    pub seqno: u16,
    /// the feasibility distance
    pub fd: Option<u16>,
    /// the next-hop address of this route, not sent to neighbours
    pub next_hop: Option<T::NodeAddress>
}