use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use crate::concepts::neighbour::Neighbour;
use crate::framework::SystemNetwork;

pub trait NetworkInterface<N: SystemNetwork> {
    fn network_type(&self) -> N::NetworkTypes;
    /// Self address of the interface
    fn address(&self) -> Box<dyn NetworkAddress<N>>;
    /// Interface Id
    fn get_id(&self) -> u16;
    /// Cost to reach an address, 0xFFFF for Infinity
    fn get_cost(&self, addr: &dyn NetworkAddress<N>) -> u16;
}

pub trait NetworkAddress<N: SystemNetwork> {
    fn network_type(&self) -> N::NetworkTypes;
    fn get_bytes(&self) -> Vec<u8>;
}

/// 3.2.3. The Interface Table (Entry)
pub struct Interface<'s, N: SystemNetwork> {
    pub net_if: Box<dyn NetworkInterface<N>>,
    pub neighbours: HashMap<Box<dyn NetworkAddress<N>>, Box<Neighbour<'s, N>>>,
    pub out_seqno: u16,
    pub timer_last_hello: Option<Instant>,
    pub timer_last_update: Option<Instant>
}