use std::cell::{RefCell, RefMut};
use std::cmp::{min, Ordering, Reverse};
use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use crate::concepts::interface::{Interface, NetworkInterface};
use crate::concepts::neighbour::Neighbour;
use crate::concepts::route::{Route, Source, SourceEntry};
use crate::concepts::tlv::{IncomingTlv, OutgoingTlv, Tlv};
use crate::framework::{RoutingSystem};

pub enum ScheduledType<'system, T : RoutingSystem> {
    RouteGC(&'system Source<T>)
}

pub struct Scheduled<'system, T: RoutingSystem>{
    pub time: Reverse<Instant>,
    pub scheduled: ScheduledType<'system, T>
}

impl<'system, T: RoutingSystem> PartialEq<Self> for Scheduled<'system, T> {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

impl<'system, T: RoutingSystem> PartialOrd for Scheduled<'system, T>{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.time.partial_cmp(&other.time)
    }
}

impl<'system, T: RoutingSystem> Eq for Scheduled<'system, T> {
}
impl<'system, T: RoutingSystem> Ord for Scheduled<'system, T>{
    fn cmp(&self, other: &Self) -> Ordering {
        self.time.cmp(&other.time)
    }
}

pub const INF: u16 = 0xFFFF;

pub struct Router<'system, T : RoutingSystem> {
    pub interfaces: Vec<Interface<'system, T>>,
    pub sources: HashMap<Source<T>, SourceEntry<T>>, // for feasibility
    pub routes: HashMap<Source<T>, Route<'system, T>>,
    pub address: T::NodeAddress,
    pub incoming: VecDeque<IncomingTlv<'system, T>>,
    pub outgoing: BinaryHeap<OutgoingTlv<'system, T>>,
    pub scheduler: BinaryHeap<Scheduled<'system, T>>
}

impl<'system, T: RoutingSystem> Router<'system, T>{
    pub fn new(router_id: u64, address: T::NodeAddress) -> Self {
        Self {
            interfaces: vec![],
            routes: HashMap::new(),
            sources: HashMap::new(),
            address,
            incoming: VecDeque::new(),
            outgoing: BinaryHeap::new(),
            scheduler: BinaryHeap::new()
        }
    }
    
    // region Interface
    fn add_interface(&mut self, interface: Box<dyn NetworkInterface<T>>) {
        for itf in &mut self.interfaces{
            if itf.net_if.id() == interface.id(){
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
    fn remove_interface(&mut self, id: T::InterfaceId){
        let pos = self.interfaces.iter().position(|x|
            x.net_if.id() == id
        );
        
        if let Some(idx) = pos {
            self.interfaces.swap_remove(idx);
        }
    }
    // endregion
    
    // region Route Selection
    pub fn update_routes(&'system mut self){
        for itf in &mut self.interfaces{
            for (_n_addr, neigh) in &mut itf.neighbours {
                let cost = neigh.get_cost();

                for (src, neigh_route) in &neigh.routes{
                    let metric = 
                        if cost == INF || neigh_route.metric == INF {
                            INF
                        }
                        else {
                            min((INF - 1) as u32, cost as u32 + neigh_route.metric as u32) as u16
                        };
                    
                    let entry = self.routes.get_mut(&src);
                    
                    // update route table if there are better entries
                    if let Some(table_route) = entry{
                        if neigh_route.seqno > table_route.seqno || (neigh_route.seqno == table_route.seqno && metric < table_route.metric){
                            table_route.next_hop = neigh.addr.clone();
                            table_route.metric = metric;
                            table_route.seqno = neigh_route.seqno;
                            table_route.neighbour = Some(&**neigh);
                        }
                    }
                    else{
                        let mut n_route = neigh_route.clone();
                        n_route.selected = true;
                        n_route.next_hop = neigh.addr.clone();
                        n_route.metric = metric;
                        n_route.neighbour = Some(&**neigh);
                        
                        self.routes.insert(src.clone(), n_route);
                    }
                }
            }
        }
    }
    // endregion
    
    // region Tlv Handling
    
    pub fn send_tlv(&mut self, tlv: Tlv, neighbour: &'system Neighbour<'system, T>, send_at: Instant){
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
        // let now = Instant::now();
        // while !self.scheduler.is_empty(){
        //     if let Some((inst, _)) = self.scheduler.peek(){
        //         if inst.0 <= now {
        //             if let Some((_, item)) = self.scheduler.pop(){
        //                 self.execute_scheduled(item);
        //                 continue;
        //             }
        //         }
        //     }
        //     break;
        // }

        todo!();
    }

    // fn execute_scheduled(&mut self, item: ScheduledType){
    //     todo!();
    // }
}