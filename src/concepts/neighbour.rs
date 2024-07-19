use std::cell::RefCell;
use std::cmp::{max, min};
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use crate::concepts::interface::{NetworkInterface, AddressType, Interface};
use anyhow::{anyhow, Context, Result};
use crate::concepts::route::{Route};
use crate::framework::{RoutingSystem};
use crate::router::{INF, Router};

/// 3.2.4. The Neighbour Table
pub struct Neighbour<T: RoutingSystem> {
    /// the physical interface where the neighbour resides
    pub itf: T::InterfaceId,
    /// the physical address of the neighbouring interface
    pub addr_phy: T::PhysicalAddress,
    /// the routing network address
    pub addr: T::NodeAddress,
    // pub hello_interval: Duration,
    // pub timer_last_ihu: Instant,
    pub routes: HashMap<T::NodeAddress, Route<T>>
}

impl <T: RoutingSystem> Neighbour<T>{
    
}

impl <T: RoutingSystem> PartialEq for Neighbour<T>{
    fn eq(&self, other: &Self) -> bool {
        // same neighbour if they share the same interface and network address
        
        self.itf == other.itf &&
            self.addr_phy == other.addr_phy &&
            self.addr == other.addr
    }
}