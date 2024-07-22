use crate::concepts::neighbour::Neighbour;
use crate::framework::RoutingSystem;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};

/// 3.2.6. The Route Table
#[derive(Clone, Serialize, Deserialize)]
pub struct Route<T: RoutingSystem> {
    /// the source and seqno for which this route is advertised
    pub source: T::MAC<Source<T>>,
    /// the interface that the neighbour is available on
    pub itf: Option<T::InterfaceId>,
    /// the metric with which this route was advertised by the neighbour, or FFFF hexadecimal (infinity) for a recently retracted route
    pub metric: u16,
    /// the feasibility distance
    pub fd: Option<u16>,
    /// the next-hop address of this route, not sent to neighbours
    pub next_hop: Option<T::NodeAddress>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Source<T: RoutingSystem> {
    pub addr: T::NodeAddress,
    pub seqno: u16,
}
