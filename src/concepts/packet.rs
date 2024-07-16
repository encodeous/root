use std::cmp::Ordering;
use std::time::Instant;
use crate::concepts::neighbour::Neighbour;
use crate::framework::{RoutingSystem};

#[derive(Eq, PartialEq, Ord, PartialOrd)]
pub enum Data<T: RoutingSystem> {
    RouteUpdate {
        /// Secured source information signed by the source (address, node-id, seqno)
        source: T::MAC<(T::NodeAddress, u16)>,
        metric: u16
    },
    RouteRequest {
        /// the source to request information for
        source: T::NodeAddress,
        dedup: [u8; 16]
    }
}

pub struct IncomingData<'system, T: RoutingSystem> {
    pub data: T::MAC<Data<T>>,
    pub neighbour: &'system Neighbour<'system, T>
}

pub struct OutgoingData<'system, T: RoutingSystem> {
    pub send_at: Instant,
    pub data: T::MAC<Data<T>>,
    pub neighbour: &'system Neighbour<'system, T>
}