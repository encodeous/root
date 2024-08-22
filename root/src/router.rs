use crate::concepts::neighbour::Neighbour;
use crate::concepts::packet::{OutboundPacket, Packet, RouteUpdate};
use crate::concepts::route::{Route, Source};
use crate::framework::{MAC, MACSignature, MACSystem, RootData, RoutingSystem};
use crate::router::UpdateAction::{NoAction, Retraction, SeqnoUpdate};
use crate::util::{increment, increment_by, seqno_less_than, sum_inf};
use log::{error};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use educe::Educe;
use serde_with::serde_as;

pub const INF: u16 = 0xFFFF;

#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Router<T: RoutingSystem + ?Sized> {
    #[serde_as(as = "Vec<(_, _)>")]
    pub links: HashMap<T::Link, Neighbour<T>>,
    /// Source, Route
    /// #[serde_as(as = "Vec<(_, _)>")]
    pub routes: HashMap<T::NodeAddress, Route<T>>,
    pub address: T::NodeAddress,
    #[serde_as(as = "Vec<(_, _)>")]
    pub seqno_requests: HashMap<T::NodeAddress, u16>,
    pub broadcast_route_for: HashSet<T::NodeAddress>,
    pub outbound_packets: Vec<OutboundPacket<T>>,
    pub seqno: u16,
    #[serde(skip_serializing, skip_deserializing)]
    pub mac_sys: T::MACSystem
}

#[derive(Eq, PartialEq)]
enum UpdateAction {
    SeqnoUpdate,
    Retraction,
    NoAction,
}

impl<T: RoutingSystem> Router<T> {
    pub fn new(address: T::NodeAddress) -> Self {
        Self{
            links: HashMap::new(),
            routes: HashMap::new(),
            address,
            seqno_requests: HashMap::new(),
            broadcast_route_for: HashSet::new(),
            outbound_packets: Vec::new(),
            seqno: 0,
            mac_sys: Default::default()
        }
    }

    /// updates the state of the router, does not broadcast routes
    pub fn update(&mut self){
        self.update_routes();
        self.broadcast_seqno_updates();
    }
    /// performs a full update on the state of the router, will broadcast routes to neighbours
    pub fn full_update(&mut self){
        self.update_routes();

        self.solve_starvation();
        self.broadcast_routes();

        self.broadcast_seqno_updates();
    }

    // region Interface
    /// writes a packet to the outbound packet queue for all neighbours
    pub fn write_broadcast_packet(&mut self, packet: &MAC<Packet<T>, T>) {
        // send to all neighbours
        for (link, neigh) in &self.links {
            self.outbound_packets.push(OutboundPacket {
                link: link.clone(),
                dest: neigh.addr.clone(),
                packet: packet.clone(),
            });
        }
    }

    fn make_self_route_for_seqno(&self, seqno: u16) -> Route<T> {
        Route {
            link: None,
            fd: None,
            metric: 0,
            next_hop: None,
            source: self.mac_sys.sign(
                Source {
                    addr: self.address.clone(),
                    seqno,
                },
                self,
            ),
        }
    }
    // endregion

    // region Route Selection

    pub fn solve_starvation(&mut self) {
        let mut packets = Vec::new();
        for (addr, route) in &self.routes {
            // check if starved
            if route.metric == INF {
                // starved
                let cur_seqno = self.get_seqno_for(addr);
                if let Some(seqno) = cur_seqno {
                    let nseqno = increment_by(seqno, 1);
                    packets.push(self.mac_sys.sign(
                        Packet::SeqnoRequest {
                            source: addr.clone(),
                            seqno: nseqno, // want to increment this at least one
                        },
                        self,
                    ));
                }
            }
        }
        for packet in packets {
            self.write_broadcast_packet(&packet);
        }
    }

    fn is_feasible(selected_route: &Route<T>, new_route: &Route<T>, metric: u16) -> Option<u16> {
        if let Some(fd) = selected_route.fd {
            let s = selected_route.source.data().seqno;
            let n = new_route.source.data().seqno;
            if seqno_less_than(n, s) {
                return None;
            }
            if metric < fd || seqno_less_than(s, n)
                || (metric == fd && selected_route.metric == INF) // TODO: Prove why this is valid, and doesnt cause issues...
            {
                return Some(metric);
            }
        }
        None
    }

    /// Recalculate routes based on current data
    pub fn update_routes(&mut self) {
        // handle route retractions
        let mut retractions = Vec::new();
        for (_addr, route) in &mut self.routes {
            if let Some(link) = &route.link {
                // this should always be true if the next hop exists
                // check if link still exists
                if !self.links.contains_key(link){
                    route.metric = INF;
                    retractions.push(route.source.clone());
                }
            }
        }
        for (link, neigh) in &self.links {
            for (src, neigh_route) in &neigh.routes {
                if *src == self.address{
                    continue; // we can safely ignore a route to ourself
                }

                let metric = sum_inf(neigh.link_cost, neigh_route.metric);
                let entry = self.routes.get_mut(src);

                // if the table has the route
                if let Some(table_route) = entry {
                    // update route table if the entry is better
                    if let Some(new_fd) = Self::is_feasible(table_route, neigh_route, metric) {
                        // we have a better route!
                        table_route.metric = metric;
                        table_route.source = neigh_route.source.clone();
                        table_route.fd = Some(new_fd);
                        table_route.link = Some(link.clone());
                        table_route.next_hop = Some(neigh.addr.clone());
                    } else if let Some(nh) = &table_route.next_hop {
                        // this is a selected route, we should update this regardless.
                        if *nh == neigh.addr {
                            // update route metric
                            if let Some(fd) = table_route.fd {
                                if metric > fd {
                                    // infeasible route, we should retract this
                                    table_route.metric = INF;
                                    retractions.push(table_route.source.clone());
                                } else {
                                    // same or better route
                                    table_route.metric = metric;
                                    table_route.fd = Some(metric);
                                }
                            }
                        }
                    }
                } else {
                    // create the new route
                    let mut n_route = neigh_route.clone();
                    n_route.next_hop = Some(neigh.addr.clone());
                    n_route.metric = metric;
                    n_route.fd = Some(metric);
                    n_route.link = Some(link.clone());

                    self.routes.insert(src.clone(), n_route);
                }
            }
        }

        for retract in retractions {
            self.write_retraction_for(retract);
        }
    }
    // endregion

    // pushes updates to neighbours
    pub fn broadcast_routes(&mut self) {
        let mut vec = Vec::new();
        for route in self.routes.values() {
            vec.push(RouteUpdate {
                source: route.source.clone(),
                metric: route.metric,
            })
        }
        vec.push(RouteUpdate{
            source: self.mac_sys.sign(
                Source {
                    addr: self.address.clone(),
                    seqno: self.seqno,
                },
                self,
            ),
            metric: 0
        });
        self.write_broadcast_packet(&self.mac_sys.sign(
            Packet::BatchRouteUpdate { routes: vec },
            self,
        ))
    }
    /// you should call this after calling update routes, otherwise the seqno metrics published is not the best...
    pub fn broadcast_seqno_updates(&mut self) {
        let tmp_seqno = self.broadcast_route_for.clone();
        for source in tmp_seqno {
            if let Some(pkt) = self.create_seqno_packet(&source) {
                self.write_broadcast_packet(&pkt);
            }
        }
        self.broadcast_route_for.clear();
    }

    /// Creates a seqno packet using the data we already have
    fn create_seqno_packet(&self, addr: &T::NodeAddress) -> Option<MAC<Packet<T>, T>> {
        if *addr == self.address{
            return Some(self.mac_sys.sign(
                Packet::UrgentRouteUpdate(RouteUpdate{
                    source: self.mac_sys.sign(
                        Source {
                            addr: self.address.clone(),
                            seqno: self.seqno,
                        },
                        self,
                    ),
                    metric: 0
                }),
                self,
            ));
        }
        if let Some(route) = self.routes.get(addr) {
            return Some(self.mac_sys.sign(
                Packet::UrgentRouteUpdate(RouteUpdate {
                    source: route.source.clone(),
                    metric: route.metric,
                }),
                self,
            ));
        }
        None
    }

    /// broadcasts a retraction for a specific source, to all neighbours
    fn write_retraction_for(&mut self, source: MAC<Source<T>, T>) {
        self.write_broadcast_packet(&self.mac_sys.sign(
            Packet::UrgentRouteUpdate(RouteUpdate {
                source,
                metric: INF,
            }),
            self,
        ))
    }

    /// handle a single packet
    pub fn handle_packet(
        &mut self,
        data: &MAC<Packet<T>, T>,
        link: &T::Link,
        neigh: &T::NodeAddress
    ) {
        if !self.mac_sys.validate(data, neigh) {
            error!(
                "Rejected packet from {}, invalid neighbour MAC. Is there a MITM attack?",
                json!(neigh)
            );
            return;
        }

        // if exists, contains the address we should broadcast
        // let mut broadcast_seqno_for: Option<T::NodeAddress> = None;

        match data.data() {
            Packet::UrgentRouteUpdate(route) => {
                // println!("[dbg] {} got packet {} from {}", json!(self.address), json!(data), json!(neigh));
                match self.handle_neighbour_route_update(route, link, neigh) {
                    SeqnoUpdate => {
                        // let's rebroadcast this change, our seqno has increased!
                        self.broadcast_route_for
                            .insert(route.source.data().addr.clone());
                    }
                    Retraction => {
                        // broadcast this retraction
                        self.write_retraction_for(route.source.clone());
                    }
                    NoAction => {}
                }
            }
            Packet::BatchRouteUpdate { routes } => {
                for route in routes {
                    self.handle_neighbour_route_update(route, link, neigh); // we dont need to worry about seqno updates and retractions
                }
            }
            Packet::SeqnoRequest { source, seqno } => {
                // if we are the node in question, we can simply increment our seqno and send it!

                // println!("[dbg] {} got packet {} from {}", json!(self.address), json!(data), json!(neigh));
                // println!("[dbg] got seqno req req_seqno={}, node={}", json!(seqno), json!(self.address));

                if let Some(cur_seqno) = self.get_seqno_for(source) {
                    if seqno_less_than(*seqno, cur_seqno) || cur_seqno == *seqno {
                        // TODO: Potentially only respond to the requester, may reduce the network traffic marginally, though may increase convergence time in higher packet loss environments

                        // we have a higher or equal seqno, yay! we can broadcast our current seqno.
                        self.broadcast_route_for.insert(source.clone());
                    } else if self.address == *source {
                        // we are the intended recipient, so we can broadcast this!
                        increment(&mut self.seqno);
                        self.broadcast_route_for.insert(self.address.clone());
                    } else {
                        let req_seqno = self.seqno_requests.entry(source.clone()).or_insert(0);
                        // prevent duplication and infinite amplification... :skull:
                        if seqno_less_than(*req_seqno, *seqno) {
                            // println!("[dbg] re-requesting seqno src={}, cur_seqno={cur_seqno}, node={}", json!(source), json!(self.address));
                            // sadge, we need to request for seqno too
                            *req_seqno = *seqno; // make sure we dont ask for this seqno again
                            self.write_broadcast_packet(&self.mac_sys.sign(
                                Packet::SeqnoRequest {
                                    source: source.clone(),
                                    seqno: *seqno,
                                },
                                self,
                            ));
                        } else {
                            // println!("[dbg] ignoring request, de-duplication");
                        }
                    }
                } else {
                    // println!("[dbg] ignoring request, we dont have seqno for requested {}", json!(source));
                }
            }
        }
    }

    pub fn get_seqno_for(&self, addr: &T::NodeAddress) -> Option<u16> {
        if *addr == self.address{
            return Some(self.seqno);
        }
        if let Some(x) = self.routes.get(addr) {
            let data = x.source.data();
            return Some(data.seqno);
        }
        None
    }

    /// handles neighbour route updates, returns true if seqno is incremented
    fn handle_neighbour_route_update(
        &mut self,
        update: &RouteUpdate<T>,
        link: &T::Link,
        neigh: &T::NodeAddress
    ) -> UpdateAction {
        let Source { addr, seqno } = update.source.data();

        if *addr == self.address{
            // just ignore this
            return NoAction;
        }

        // validate update
        if !self.mac_sys.validate(&update.source, addr) {
            error!(
                "Rejected route update for {} from {}, invalid source MAC. Is there a MITM attack?",
                json!(addr),
                json!(neigh)
            );
            return NoAction;
        }

        let mut action = NoAction;
        let stored_seqno = self.get_seqno_for(addr);
        if let Some(d_seqno) = stored_seqno {
            if seqno_less_than(*seqno, d_seqno) {
                return NoAction; // our neighbour is probably out of date. seqno cannot decrease
            } else if seqno_less_than(d_seqno, *seqno) {
                action = SeqnoUpdate; // our neighbour has a higher seqno than us!
            }
        }
        
        // check if this route is the currently selected route
        let mut selected = false;
        if let Some(route) = self.routes.get(addr){
            if let Some(nh) = &route.next_hop{
                selected = nh == neigh;
            }
        }

        if let Some(neighbour) = self.links.get_mut(link) {
            // update the value
            if let Some(entry) = neighbour.routes.get_mut(addr) { // neighbour entry exists!
                entry.source = update.source.clone();
                if update.metric == INF && entry.metric != INF && selected {
                    // this is a retraction!
                    if action != NoAction {
                        error!("Unexpected state: A seqno increase should not have a metric of INF!")
                    }
                    action = Retraction;
                }
                entry.metric = update.metric;
            } else if update.metric != INF || selected {
                // we add the route if it is not INF, and it is not the next hop
                let route = Route {
                    source: update.source.clone(),
                    metric: update.metric,
                    link: None,
                    fd: None,
                    next_hop: None,
                };
                neighbour.routes.insert(addr.clone(), route);
            }
        }
        action
    }
}

#[derive(Default)]
pub struct NoMACSystem {

}

#[derive(Serialize, Deserialize, Educe)]
#[educe(Clone(bound()))]
#[serde(bound = "")]
pub struct DummyMAC<V: RootData>{
    pub data: V,
}

impl<V: RootData> From<V> for DummyMAC<V>{
    fn from(value: V) -> Self {
        Self{
            data: value
        }
    }
}

impl<V: RootData, T: RoutingSystem + ?Sized> MACSignature<V, T> for DummyMAC<V>{
    fn data(&self) -> &V {
        &self.data
    }

    fn data_mut(&mut self) -> &mut V {
        &mut self.data
    }
}

impl<T: RoutingSystem + ?Sized> MACSystem<T> for NoMACSystem {
    type MACSignatureType<V: RootData> = DummyMAC<V>;
    fn sign<V: RootData>(&self, data: V, router: &Router<T>) -> DummyMAC<V>{
        DummyMAC{
            data
        }
    }

    fn validate<V: RootData>(&self, sig: &MAC<V, T>, subject: &T::NodeAddress) -> bool {
        true
    }
}