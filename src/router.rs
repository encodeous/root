use std::cell::{RefCell, RefMut};
use std::cmp::{min, Ordering, Reverse};
use std::collections::{HashMap, VecDeque};
use crate::concepts::interface::{Interface, NetworkInterface};
use crate::concepts::neighbour::Neighbour;
use crate::concepts::route::{Route};
use crate::concepts::packet::{IncomingData, OutgoingData, Data};
use crate::framework::{RoutingSystem};
use crate::util::{seqno_less_than, sum_inf};

// pub enum ScheduledType<'system, T : RoutingSystem> {
//     RouteGC(&'system Source<T>)
// }
// 
// pub struct Scheduled<'system, T: RoutingSystem>{
//     pub time: Reverse<Instant>,
//     pub scheduled: ScheduledType<'system, T>
// }
// 
// impl<'system, T: RoutingSystem> PartialEq<Self> for Scheduled<'system, T> {
//     fn eq(&self, other: &Self) -> bool {
//         self.time == other.time
//     }
// }
// 
// impl<'system, T: RoutingSystem> PartialOrd for Scheduled<'system, T>{
//     fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
//         self.time.partial_cmp(&other.time)
//     }
// }
// 
// impl<'system, T: RoutingSystem> Eq for Scheduled<'system, T> {
// }
// impl<'system, T: RoutingSystem> Ord for Scheduled<'system, T>{
//     fn cmp(&self, other: &Self) -> Ordering {
//         self.time.cmp(&other.time)
//     }
// }

pub const INF: u16 = 0xFFFF;

pub struct Router<'system, T : RoutingSystem> {
    pub interfaces: HashMap<T::InterfaceId, Interface<'system, T>>,
    pub routes: HashMap<T::NodeAddress, Route<'system, T>>,
    pub address: T::NodeAddress,
    pub incoming: VecDeque<IncomingData<'system, T>>,
    pub outgoing: VecDeque<OutgoingData<'system, T>>,
    // pub scheduler: BinaryHeap<Scheduled<'system, T>>
}

impl<'system, T: RoutingSystem> Router<'system, T>{
    pub fn new(address: T::NodeAddress) -> Self {
        Self {
            interfaces: HashMap::new(),
            routes: HashMap::new(),
            address,
            incoming: VecDeque::new(),
            outgoing: VecDeque::new(),
            // scheduler: BinaryHeap::new()
        }
    }
    
    // region Interface
    pub fn add_interface(&mut self, interface: Box<dyn NetworkInterface<T>>) {
        for (id, itf) in &mut self.interfaces{
            if *id == interface.id(){
                // interface exists
                return;
            }
        }
        let n_itf = Interface{
            net_if: interface,
            neighbours: Default::default()
        };
        self.interfaces.insert(n_itf.net_if.id(), n_itf);
    }
    pub fn remove_interface(&mut self, id: T::InterfaceId){
        self.interfaces.retain(|itf_id, _| *itf_id != id);
    }
    
    /// Queries the physical network interfaces for neighbours
    pub fn refresh_interfaces(&mut self) {
        for (id, itf) in &mut self.interfaces{
            // pull data from network interfaces
            for (phy, addr) in itf.net_if.get_neighbours() {
                {
                    if let Some(val) = itf.neighbours.get_mut(&addr) {
                        if val.addr_phy == phy {
                            continue; // ok, the net address didn't change
                        }
                        // the address changed!!!
                        itf.neighbours.remove(&addr); // we need to replace this!
                        // remove any routes containing the neighbour
                        self.routes.retain(|r_addr, route| {
                            if let Some(nh_addr) = &route.next_hop {
                                if nh_addr == &addr {
                                    return false;
                                }
                            }
                            true
                        });
                    }
                    // add the neighbour
                }
                let neigh = Neighbour {
                    itf: id.clone(),
                    addr_phy: phy,
                    addr: addr.clone(),
                    routes: HashMap::new(),
                };
                itf.neighbours.insert(addr.clone(), Box::new(neigh));
            }
        }
    }
    // endregion
    
    // region Route Selection

    fn is_feasible(selected_route: &Route<T>, new_route: &Route<T>, metric: u16) -> Option<u16>{
        let new_metric = sum_inf(new_route.metric, metric);
        if let Some(fd) = selected_route.fd{
            if new_metric < fd || seqno_less_than(selected_route.seqno, new_route.seqno) {
                return Some(new_metric);
            }
        }
        None
    }
    
    /// Recalculate metrics based on current data
    pub fn update_routes(&'system mut self){
        for (id, itf) in &mut self.interfaces{
            for neigh in itf.neighbours.values_mut() {
                let cost = itf.net_if.get_cost(&neigh.addr_phy);

                for (src, neigh_route) in &neigh.routes{
                    let metric = sum_inf(cost, neigh_route.metric);
                    
                    let entry = self.routes.get_mut(&src);

                    // update route table if there are better entries
                    if let Some(table_route) = entry{
                        if neigh_route.seqno < table_route.seqno {
                            // this route is outdated
                            continue;
                        }
                        if let Some(new_fd) = Self::is_feasible(table_route, neigh_route, metric){
                            // we have a better route!
                            table_route.next_hop = Some(neigh.addr.clone());
                            table_route.metric = metric;
                            table_route.seqno = neigh_route.seqno;
                            table_route.neighbour = Some(&**neigh);
                            table_route.fd = Some(new_fd);
                            table_route.itf = Some(id.clone());
                        }
                    }
                    else{
                        // selected
                        let mut n_route = neigh_route.clone();
                        n_route.next_hop = Some(neigh.addr.clone());
                        n_route.metric = metric;
                        n_route.fd = Some(metric);
                        n_route.neighbour = Some(&**neigh);
                        
                        self.routes.insert(src.clone(), n_route);
                    }
                }
            }
        }
    }
    // endregion
}