use std::cell::{RefCell, RefMut};
use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use crate::concepts::interface::{Interface, NetworkInterface};
use crate::concepts::neighbour::Neighbour;
use crate::concepts::route::{Route, Source, SourceEntry};
use crate::concepts::tlv::{IncomingTlv, OutgoingTlv, Tlv};
use crate::framework::{SystemNetwork, Routing, Persistence};

#[derive(Ord, PartialOrd, Eq, PartialEq)]
pub enum Scheduled{

}

pub struct PersistentStorage{
    pub node_seqno: u16,
    pub router_id: u64,
}

pub const INF: u16 = 0xFFFF;

pub struct Router<'system, R: Routing, N: SystemNetwork, P: Persistence> {
    pub interfaces: Vec<Interface<'system, N>>,
    pub sources: HashMap<Source<R>, SourceEntry<R>>, // for feasibility
    pub routes: HashMap<Source<R>, Route<'system, R, N>>,
    pub address: R::AddressType,
    pub incoming: VecDeque<IncomingTlv<'system, N>>,
    pub outgoing: BinaryHeap<OutgoingTlv<'system, N>>,
    pub scheduler: BinaryHeap<(Reverse<Instant>, Scheduled)>,
    pub store: PersistentStorage
}

impl<'system, R: Routing, N: SystemNetwork, P: Persistence> Router<'system, R, N, P>{
    pub fn restore(store: PersistentStorage, address: R::AddressType) -> Self {
        Self {
            interfaces: vec![],
            routes: HashMap::new(),
            sources: HashMap::new(),
            address,
            incoming: VecDeque::new(),
            outgoing: BinaryHeap::new(),
            scheduler: BinaryHeap::new(),
            store
        }
    }

    pub fn new(router_id: u64, address: R::AddressType) -> Self {
        Self {
            interfaces: vec![],
            routes: HashMap::new(),
            sources: HashMap::new(),
            address,
            incoming: VecDeque::new(),
            outgoing: BinaryHeap::new(),
            scheduler: BinaryHeap::new(),
            store: PersistentStorage{
                node_seqno: 0,
                router_id
            }
        }
    }
    
    // region Interface
    fn add_interface(&mut self, interface: Box<dyn NetworkInterface<N>>) {
        for itf in &mut self.interfaces{
            if itf.net_if.get_id() == interface.get_id(){
                // interface exists
                return;
            }else{
                
            }
        }
        let n_itf = Interface{
            net_if: interface,
            neighbours: Default::default(),
            out_seqno: 0,
            timer_last_hello: None,
            timer_last_update: None,
        };
        self.interfaces.push(n_itf)
    }
    fn remove_interface(&mut self, id: Vec<u8>){
        let pos = self.interfaces.iter().position(|x|
            x.net_if.get_id() == id
        );
        
        if let Some(idx) = pos {
            self.interfaces.swap_remove(idx);
        }
    }
    // endregion
    
    // region Route Selection
    pub fn route_selection(&mut self){
        // let src_table = HashMap::<()>
        for itf in &mut self.interfaces{
            for (n_addr, neigh) in &mut itf.neighbours {
                let k = n_addr.network_type() as u8;
                let cost = neigh.get_cost();
                for entry in &mut self.routes{
                    let addr = &entry.neighbour;
                    
                }
            }
        }
    }
    // endregion
    
    // region Tlv Handling
    
    pub fn send_tlv(&mut self, tlv: Tlv, neighbour: &'system Neighbour<'system, N>, send_at: Instant){
        self.outgoing.push(OutgoingTlv {
            tlv,
            neighbour,
            send_at,
        })
    }
    
    // endregion
    
    /// Call this function regularly to update incoming and outgoing, and to execute all operations in the protocol.
    /// returns: When to next call the tick function
    pub fn tick(&mut self) -> Instant {
        // run scheduled items
        let now = Instant::now();
        while !self.scheduler.is_empty(){
            if let Some((inst, _)) = self.scheduler.peek(){
                if inst.0 <= now {
                    if let Some((_, item)) = self.scheduler.pop(){
                        self.execute_scheduled(item);
                        continue;
                    }
                }
            }
            break;
        }

        todo!();
    }

    fn execute_scheduled(&mut self, item: Scheduled){
        todo!();
    }
}