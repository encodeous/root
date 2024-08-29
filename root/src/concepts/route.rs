use crate::framework::{MAC, RoutingSystem};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use educe::Educe;

#[derive(Educe)]
#[educe(Clone(bound()))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize), serde(bound = ""))]
pub struct Route<T: RoutingSystem + ?Sized> {
    /// the source and seqno for which this route is advertised
    pub source: MAC<Source<T>, T>,
    /// the metric with which this route was advertised by the neighbour, or FFFF hexadecimal (infinity) for a recently retracted route
    pub metric: u16,
    /// the feasibility distance
    pub fd: u16,
    /// the physical link that connects to the next hop
    pub link: T::Link,
    /// the next-hop address of this route
    pub next_hop: T::NodeAddress,
    /// whether this route has been retracted, if it has, do not retract again
    pub retracted: bool
}

#[derive(Educe)]
#[educe(Clone(bound()))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize), serde(bound = ""))]
pub struct ExternalRoute<T: RoutingSystem + ?Sized> {
    /// the source and seqno for which this route is advertised
    pub source: MAC<Source<T>, T>,
    /// the metric with which this route was advertised by the neighbour, or FFFF hexadecimal (infinity) for a recently retracted route
    pub metric: u16,
    /// whether this route has been retracted, if it has, do not retract again
    pub retracted: bool
}

#[derive(Educe)]
#[educe(Clone(bound()))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize), serde(bound = ""))]
pub struct Source<T: RoutingSystem + ?Sized> {
    pub addr: T::NodeAddress,
    pub seqno: u16,
}
