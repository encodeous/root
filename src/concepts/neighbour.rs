use std::cell::RefCell;
use std::cmp::{max, min};
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use crate::concepts::interface::{NetworkInterface, NetworkAddress, Interface};
use anyhow::{anyhow, Context, Result};
use crate::concepts::tlv::Tlv;
use crate::framework::{SystemNetwork, Routing};
use crate::router::{INF, Router};

/// 3.2.4. The Neighbour Table
pub struct Neighbour<'owner, N: SystemNetwork> {
    /// the local node's interface over which this neighbour is reachable
    interface: &'owner Interface<'owner, N>,
    /// the address of the neighbouring interface
    address: Box<dyn NetworkAddress<N>>,
    // TODO: history of hello packets,
    /// the "transmission cost" value from the last IHU packet received from this neighbour, or FFFF hexadecimal (infinity) if the IHU hold timer for this neighbour has expired
    transmission_cost: u16,
    seqno_in: u16,
    seqno_out: u16,
    last_hello: Instant,
    hello_interval: Duration,
    timer_last_ihu: Instant,
}

impl <'owner, N: SystemNetwork> Neighbour<'owner, N>{
    pub fn process_hello(&mut self, router: &mut Router<impl Routing, N>, tlv: Tlv) -> Result<()>{
        if let Tlv::Hello {interval_ms} = tlv{
            self.last_hello = Instant::now();
            self.hello_interval = Duration::from_millis(interval_ms as u64);
            router.route_selection();
            return Ok(())
        }
        Err(anyhow!("Expected Hello Tlv"))
    }

    pub fn get_cost(&self) -> u16{
        self.interface.net_if.get_cost(self.address.as_ref())
    }

    // region TLV Handling
    fn handle_tlv(&mut self, tlv: &Tlv, router: &mut Router<impl Routing, N>){
        match tlv {
            Tlv::Hello{interval_ms} => {
                self.process_hello(tlv.d, router);
            },
            _ => {
                // unhandled tlv, TODO handling
            },
        }
    }

    // endregion
}

impl <'owner, N: SystemNetwork> PartialEq for Neighbour<'owner, N>{
    fn eq(&self, other: &Self) -> bool {
        // same neighbour if they share the same interface and network address
        self.interface.net_if.get_id() == other.interface.net_if.get_id() &&
            self.address.get_bytes() == other.address.get_bytes()
    }
}