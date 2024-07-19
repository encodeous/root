use std::hash::{Hash, Hasher};
use crate::concepts::neighbour::Neighbour;
use crate::framework::{RoutingSystem};

/// 3.2.6. The Route Table
#[derive(Clone)]
pub struct Route<T: RoutingSystem> {
    /// the source and seqno for which this route is advertised
    pub source: T::MAC<(T::NodeAddress, u16)>,
    /// the interface that the neighbour is available on
    pub itf: Option<T::InterfaceId>,
    /// the metric with which this route was advertised by the neighbour, or FFFF hexadecimal (infinity) for a recently retracted route
    pub metric: u16,
    /// the feasibility distance
    pub fd: Option<u16>,
    /// the next-hop address of this route, not sent to neighbours
    pub next_hop: Option<T::NodeAddress>
}