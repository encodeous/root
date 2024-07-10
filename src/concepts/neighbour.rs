use std::cell::RefCell;
use std::cmp::{max, min};
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use crate::concepts::interface::{NetworkInterface, AddressType, Interface};
use anyhow::{anyhow, Context, Result};
use crate::concepts::route::{Route, Source};
use crate::concepts::tlv::Tlv;
use crate::framework::{RoutingSystem};
use crate::router::{INF, Router};

/// 3.2.4. The Neighbour Table
pub struct Neighbour<'owner, T: RoutingSystem> {
    /// the local node's interface over which this neighbour is reachable
    pub interface: &'owner Interface<'owner, T>,
    /// the address of the neighbouring interface
    pub net_addr: T::NetworkAddress,
    pub addr: T::NodeAddress,
    // TODO: history of hello packets,
    /// the "transmission cost" value from the last IHU packet received from this neighbour, or FFFF hexadecimal (infinity) if the IHU hold timer for this neighbour has expired
    pub transmission_cost: u16,
    pub seqno_in: u16,
    pub seqno_out: u16,
    pub last_hello: Instant,
    pub hello_interval: Duration,
    pub timer_last_ihu: Instant,
    pub routes: HashMap<Source<T>, Route<'owner, T>>
}

impl <'owner, T: RoutingSystem> Neighbour<'owner, T>{
    pub fn process_hello(&mut self, router: &'owner mut Router<'owner, T>, tlv: Tlv) -> Result<()>{
        if let Tlv::Hello {interval_ms} = tlv{
            self.last_hello = Instant::now();
            self.hello_interval = Duration::from_millis(interval_ms as u64);
            router.update_routes();
            return Ok(())
        }
        Err(anyhow!("Expected Hello Tlv"))
    }

    pub fn get_cost(&self) -> u16{
        self.interface.net_if.get_cost(&self.net_addr)
    }

    // region TLV Handling
    fn handle_tlv(&mut self, tlv: &Tlv, router: &mut Router<T>){
        // match tlv {
        //     Tlv::Hello{interval_ms} => {
        //         self.process_hello(tlv.d, router);
        //     },
        //     _ => {
        //         // unhandled tlv, TODO handling
        //     },
        // }
    }

    // endregion
}

impl <'owner, T: RoutingSystem> PartialEq for Neighbour<'owner, T>{
    fn eq(&self, other: &Self) -> bool {
        // same neighbour if they share the same interface and network address
        
        self.interface.net_if.id() == other.interface.net_if.id() &&
            self.net_addr == other.net_addr
    }
}