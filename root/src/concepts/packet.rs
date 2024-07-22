use crate::concepts::route::Source;
use crate::framework::RoutingSystem;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(bound = "")]
pub enum Packet<T: RoutingSystem> {
    /// this is a single, unscheduled update that should be sent immediately.
    UrgentRouteUpdate(RouteUpdate<T>),
    /// this is a batch, full-table update that should only be sent periodically to all nodes
    BatchRouteUpdate { routes: Vec<RouteUpdate<T>> },
    SeqnoRequest {
        /// the source to request information for
        source: T::NodeAddress,
        /// the seqno of the request
        seqno: u16,
    },
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(bound = "")]
pub struct RouteUpdate<T: RoutingSystem> {
    /// Secured source information signed by the source (address, seqno)
    pub source: T::MAC<Source<T>>,
    pub metric: u16,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(bound = "")]
pub struct OutboundPacket<T: RoutingSystem> {
    /// send via this interface
    pub itf: T::InterfaceId,
    // to this destination
    pub addr_phy: T::PhysicalAddress,
    pub packet: T::MAC<Packet<T>>,
}
