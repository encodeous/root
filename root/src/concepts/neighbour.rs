use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::concepts::route::Route;
use crate::framework::RoutingSystem;

#[derive(Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Neighbour<T: RoutingSystem + ?Sized> {
    /// the physical interface where the neighbour resides
    pub itf: T::InterfaceId,
    /// the physical address of the neighbouring interface
    pub addr_phy: T::PhysicalAddress,
    /// the routing network address
    pub addr: T::NodeAddress,
    // pub hello_interval: Duration,
    // pub timer_last_ihu: Instant,
    pub routes: HashMap<T::NodeAddress, Route<T>>,
    /// Direct Link-cost to this neighbour, 0xFFFF for Infinity. Lower is better.
    /// INF if the link is down
    pub link_cost: u16
}

impl<T: RoutingSystem + ?Sized> Neighbour<T> {}

impl<T: RoutingSystem + ?Sized> PartialEq for Neighbour<T> {
    fn eq(&self, other: &Self) -> bool {
        // same neighbour if they share the same interface and network address

        self.itf == other.itf && self.addr_phy == other.addr_phy && self.addr == other.addr
    }
}
