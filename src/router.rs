use std::cmp::min;
use std::collections::{HashMap, HashSet};
use std::ops::DerefMut;
use std::time::Instant;
use log::{error, trace, warn};
use crate::concepts::interface::{Interface, NetworkInterface};
use crate::concepts::neighbour::Neighbour;
use crate::concepts::packet::{Packet, RouteUpdate};
use crate::concepts::route::{Route};
use crate::framework::{MACSystem, RoutingSystem};
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

pub struct Router<T : RoutingSystem> {
    pub interfaces: HashMap<T::InterfaceId, Interface<T>>,
    pub routes: HashMap<T::NodeAddress, Route<T>>,
    pub address: T::NodeAddress,
    /// a set of nodes to broadcast updates for
    pub broadcast_update: HashSet<T::NodeAddress>,
    /// history of broadcast updates
    pub dedup_update: HashMap<T::NodeAddress, u16>,
    pub dedup_seqno_request: HashMap<T::NodeAddress, Instant>,
}

impl<T: RoutingSystem> Router<T>{
    pub fn new(address: T::NodeAddress) -> Self {
        Self {
            interfaces: HashMap::new(),
            routes: HashMap::new(),
            address,
            dedup_update: HashMap::new(),
            broadcast_update: HashSet::new(),
            dedup_seqno_request: HashMap::new()
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

    /// only call once
    pub fn init(&mut self){
        // create a self route
        self.routes.insert(self.address.clone(), self.make_self_route_for_seqno(0));
    }

    fn make_self_route_for_seqno(&self, seqno: u16) -> Route<T>{
        Route{
            itf: None,
            fd: None,
            metric: 0,
            next_hop: None,
            source: T::MAC::sign((self.address.clone(), seqno), self)
        }
    }
    
    /// Queries the physical network interfaces for neighbours
    pub fn refresh_interfaces(&mut self) {
        for (id, itf) in &mut self.interfaces{
            // pull data from network interfaces
            itf.neighbours.retain(|_,v| itf.net_if.is_connected(&v.addr_phy));
            for (phy, addr) in itf.net_if.get_neighbours() {
                if let Some(val) = itf.neighbours.get_mut(&addr) {
                    if val.addr_phy == phy {
                        continue; // ok, the net address didn't change
                    }
                    // the address changed!!!
                    trace!("Network addr of neighbour {addr} changed from {} to {phy}", val.addr_phy);
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
        if let Some(fd) = selected_route.fd{
            let (_, s_seqno) = selected_route.source.data();
            let (_, n_seqno) = new_route.source.data();
            if seqno_less_than(*n_seqno, *s_seqno){
                return None;
            }
            if metric < fd || seqno_less_than(*s_seqno, *n_seqno) {
                return Some(metric);
            }
        }
        None
    }
    
    /// Recalculate metrics based on current data
    pub fn update_routes(&mut self){
        for (id, itf) in &mut self.interfaces{
            for neigh in itf.neighbours.values_mut() {
                let cost = itf.net_if.get_cost(&neigh.addr_phy);

                for (src, neigh_route) in &neigh.routes{
                    let metric = sum_inf(cost, neigh_route.metric);
                    
                    let entry = self.routes.get_mut(src);

                    // update route table if there are better entries
                    if let Some(table_route) = entry{
                        if let Some(new_fd) = Self::is_feasible(table_route, neigh_route, metric){
                            // we have a better route!
                            table_route.next_hop = Some(neigh.addr.clone());
                            table_route.metric = metric;
                            table_route.source = neigh_route.source.clone();
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
                        n_route.itf = Some(id.clone());
                        
                        self.routes.insert(src.clone(), n_route);
                    }
                }
            }
        }
    }
    // endregion

    // pushes updates to neighbours
    pub fn batch_update(&self) -> T::MAC<Packet<T>>{
        let mut vec = Vec::new();
        for (addr, route) in &self.routes{
            vec.push(RouteUpdate{
                source: route.source.clone(),
                metric: route.metric
            })
        }
        T::MAC::sign(Packet::BatchRouteUpdate {
            routes: vec
        }, self)
    }
    pub fn handle_packet(&mut self, data: &T::MAC<Packet<T>>, itf: &T::InterfaceId, neigh: &T::NodeAddress){
        if !data.validate(&neigh) {
            error!("Rejected packet from {}, invalid neighbour MAC. Is there a MITM attack?", neigh);
            return;
        }

        match data.data() {
            Packet::RouteUpdate(route) => {
                self.handle_neighbour_route_update(route, itf, neigh, true);
            }
            Packet::BatchRouteUpdate { routes } => {
                for route in routes {
                    self.handle_neighbour_route_update(route, itf, neigh, false);
                }
            }
            Packet::RouteRequest { source, dedup } => {

            }
        }
    }

    fn handle_neighbour_route_update(&mut self, update: &RouteUpdate<T>, itf: &T::InterfaceId, neigh: &T::NodeAddress, mut broadcast: bool){
        let (src_addr, seqno) = update.source.data();

        // validate update
        if !update.source.validate(src_addr) {
            error!("Rejected route update for {} from {}, invalid source MAC. Is there a MITM attack?", src_addr, neigh);
            return;
        }

        // deduplicate the request
        if let Some(d_seqno) = self.dedup_update.get(src_addr){
            if seqno_less_than(*seqno, *d_seqno) || d_seqno == seqno{
                // duplicate, do not re-broadcast
                broadcast = false;
            }
        }

        if broadcast && self.get_neighbour(itf, neigh).is_some(){
            self.dedup_update.insert(src_addr.clone(), *seqno);
            self.broadcast_update.insert(src_addr.clone());
        }

        if let Some(neighbour) = self.get_neighbour_mut(itf, neigh) {
            // update the value
            if let Some(entry) = neighbour.routes.get_mut(src_addr) {
                entry.source = update.source.clone();
                entry.metric = update.metric;
            }
            else{
                let route = Route{
                    source: update.source.clone(),
                    metric: update.metric,
                    itf: None,
                    fd: None,
                    next_hop: None
                };
                neighbour.routes.insert(src_addr.clone(), route);
            }
        }
    }


    pub fn get_neighbour(&self, itf: &T::InterfaceId, neigh: &T::NodeAddress) -> Option<&Neighbour<T>>{
        if let Some(interface) = self.interfaces.get(itf){
            if let Some(neighbour) = interface.neighbours.get(neigh){
                return Some(neighbour)
            }
        }
        None
    }
    pub fn get_neighbour_mut(&mut self, itf: &T::InterfaceId, neigh: &T::NodeAddress) -> Option<&mut Neighbour<T>>{
        if let Some(interface) = self.interfaces.get_mut(itf){
            if let Some(neighbour) = interface.neighbours.get_mut(neigh){
                return Some(neighbour)
            }
        }
        None
    }

    /// Call this function frequently
    pub fn cleanup(&mut self) -> Instant{
        let now = Instant::now();
        let mut next = now + T::config().cleanup_timer;

        // cleanup dedup
        let ttl = T::config().dedup_ttl;
        self.dedup_seqno_request.retain(|_, &mut v| {
            next = min(next, v);
            v + ttl < now
        });

        next
    }
}