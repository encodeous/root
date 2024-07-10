use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::Hash;
use std::time::Instant;
use crate::concepts::neighbour::Neighbour;
use crate::framework::{RoutingSystem};

pub trait NetworkInterface<T: RoutingSystem> {
    /// Self address of the interface
    fn address(&self) -> T::NetworkAddress;
    fn address_type(&self) -> T::NetworkType;
    // unique identifier for this interface
    fn id(&self) -> T::InterfaceId;
    /// Cost to reach an address, 0xFFFF for Infinity
    fn get_cost(&self, addr: &T::NetworkAddress) -> u16;
}

pub trait AddressType<T: RoutingSystem> {
    fn get_network_type(&self) -> T::NetworkType;
}

/// 3.2.3. The Interface Table (Entry)
pub struct Interface<'s, T: RoutingSystem> {
    pub net_if: Box<dyn NetworkInterface<T>>,
    pub neighbours: HashMap<T::NetworkAddress, Box<Neighbour<'s, T>>>,
    pub out_seqno: u16,
    pub timer_last_hello: Option<Instant>,
    pub timer_last_update: Option<Instant>
}