use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::concepts::route::{ExternalRoute};
use crate::framework::RoutingSystem;

#[derive(Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Neighbour<T: RoutingSystem + ?Sized> {
    /// the physical network link id, the pair (link, addr) should be unique
    pub link: T::Link,
    /// the routing network address
    pub addr: T::NodeAddress,
    // pub hello_interval: Duration,
    // pub timer_last_ihu: Instant,
    pub routes: HashMap<T::NodeAddress, ExternalRoute<T>>,
    /// Direct Link-cost to this neighbour, 0xFFFF for Infinity. Lower is better.
    /// INF if the link is down
    pub link_cost: u16
}