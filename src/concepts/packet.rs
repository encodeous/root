use std::fmt;
use std::fmt::{Display, Formatter, Pointer};
use std::time::Instant;
use serde::{Deserialize, Serialize};
use crate::concepts::neighbour::Neighbour;
use crate::concepts::route::Source;
use crate::framework::{RoutingSystem};

#[derive(Clone, Serialize, Deserialize)]
#[serde(bound = "")]
pub enum Packet<T: RoutingSystem> {
    /// this is a single, unscheduled update that should be sent immediately.
    SeqnoUpdate(RouteUpdate<T>),
    /// this is a batch, full-table update that should only be sent periodically to all nodes
    BatchRouteUpdate {
        routes: Vec<RouteUpdate<T>>
    },
    SeqnoRequest {
        /// the source to request information for
        source: T::NodeAddress,
        /// the seqno of the request
        seqno: u16
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(bound = "")]
pub struct RouteUpdate<T: RoutingSystem> {
    /// Secured source information signed by the source (address, seqno)
    pub source: T::MAC<Source<T>>,
    pub metric: u16
}

