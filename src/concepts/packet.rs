use std::time::Instant;
use serde::{Deserialize, Serialize};
use crate::concepts::neighbour::Neighbour;
use crate::framework::{RoutingSystem};

#[derive(Clone)]
pub enum Packet<T: RoutingSystem> {
    /// this is a single, unscheduled update that should be sent immediately.
    RouteUpdate(RouteUpdate<T>),
    /// this is a batch, full-table update that should only be sent periodically to all nodes
    BatchRouteUpdate {
        routes: Vec<RouteUpdate<T>>
    },
    RouteRequest {
        /// the source to request information for
        source: T::NodeAddress,
        dedup: [u8; 16]
    }
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RouteUpdate<T: RoutingSystem> {
    /// Secured source information signed by the source (address, seqno)
    pub source: T::MAC<(T::NodeAddress, u16)>,
    pub metric: u16
}