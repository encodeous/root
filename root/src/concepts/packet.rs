use educe::Educe;
use serde::{Deserialize, Serialize};
use crate::concepts::route::Source;
use crate::framework::{MAC, RoutingSystem};

#[derive(Serialize, Deserialize, Educe)]
#[educe(Clone(bound()))]
#[serde(bound = "")]
pub enum Packet<T: RoutingSystem + ?Sized> {
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

#[derive(Serialize, Deserialize, Educe)]
#[educe(Clone(bound()))]
#[serde(bound = "")]
pub struct RouteUpdate<T: RoutingSystem + ?Sized> {
    /// Secured source information signed by the source (address, seqno)
    pub source: MAC<Source<T>, T>,
    pub metric: u16,
}

#[derive(Serialize, Deserialize, Educe)]
#[serde(bound = "")]
#[educe(Clone(bound()))]
pub struct OutboundPacket<T: RoutingSystem + ?Sized> {
    /// send via this link
    pub link: T::Link,
    pub dest: T::NodeAddress,
    pub packet: MAC<Packet<T>, T>,
}
