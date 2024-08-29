use crate::concepts::neighbour::Neighbour;
use crate::concepts::packet::{OutboundPacket, Packet, RouteUpdate};
use crate::concepts::route::{ExternalRoute, Route, Source};
use crate::framework::{MAC, MACSignature, MACSystem, RootData, RoutingSystem};
use crate::router::UpdateAction::{NoAction, Retraction, SeqnoUpdate};
use crate::util::{increment, increment_by, seqno_less_than, sum_inf};
use std::collections::{HashMap, HashSet, VecDeque};
use cfg_if::cfg_if;
use educe::Educe;
use crate::feedback::{RoutingError, RoutingWarning};
use crate::feedback::RoutingError::MACValidationFail;
use crate::feedback::RoutingWarning::{DesynchronizedSeqno, MetricIsZero};

cfg_if!{
    if #[cfg(feature = "serde")] {
        use serde::{Deserialize, Serialize};
        use serde_with::serde_as;
    }
}

pub const INF: u16 = 0xFFFF;

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize), serde(bound = ""), serde_as)]
pub struct Router<T: RoutingSystem + ?Sized> {
    #[cfg_attr(feature = "serde", serde(skip_serializing, skip_deserializing))]
    pub links: HashMap<T::Link, Neighbour<T>>,
    /// Source, Route
    /// #[serde_as(as = "Vec<(_, _)>")]
    pub routes: HashMap<T::NodeAddress, Route<T>>,
    pub address: T::NodeAddress,
    #[cfg_attr(feature = "serde", serde_as(as = "Vec<(_, _)>"))]
    pub seqno_requests: HashMap<T::NodeAddress, u16>,
    pub broadcast_route_for: HashSet<T::NodeAddress>,
    pub outbound_packets: Vec<OutboundPacket<T>>,
    pub seqno: u16,
    #[cfg_attr(feature = "serde", serde(skip_serializing, skip_deserializing))]
    pub mac_sys: T::MACSystem,
    /// drain this regularly for warnings
    #[cfg_attr(feature = "serde", serde(skip_serializing, skip_deserializing))]
    pub warnings: VecDeque<RoutingWarning<T>>
}

#[derive(Eq, PartialEq)]
enum UpdateAction {
    SeqnoUpdate,
    Retraction,
    NoAction,
}

impl<T: RoutingSystem> Router<T> {
    fn warn(&mut self, warning: RoutingWarning<T>){
        if self.warnings.len() > T::MAX_WARN_LENGTH{
            self.warnings.pop_front();
        }
        self.warnings.push_back(warning);
    }
    pub fn new(address: T::NodeAddress) -> Self {
        Self{
            links: HashMap::new(),
            routes: HashMap::new(),
            address,
            seqno_requests: HashMap::new(),
            broadcast_route_for: HashSet::new(),
            outbound_packets: Vec::new(),
            seqno: 0,
            mac_sys: Default::default(),
            warnings: Default::default()
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

    fn is_feasible(selected_route: &Route<T>, new_route: &ExternalRoute<T>, metric: u16) -> Option<u16> {
        let fd = selected_route.fd;
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
        None
    }

    /// Recalculate routes based on current data
    pub fn update_routes(&mut self) {
        // handle route retractions
        let mut retractions = Vec::new();
        for (_addr, route) in &mut self.routes {
            let link = &route.link;
            // check if link still exists
            if !self.links.contains_key(link) || self.links.get(link).unwrap().link_cost == INF{
                route.metric = INF;
                if !route.retracted{
                    retractions.push(route.source.clone());
                }
                route.retracted = true;
            }
        }
        for (link, neigh) in &mut self.links {
            if neigh.link_cost == 0 {
                self.warnings.push_back(MetricIsZero {link: link.clone()});
                neigh.link_cost = 1;
            }
            for (src, neigh_route) in &neigh.routes {
                if *src == self.address{
                    continue; // we can safely ignore a route to ourself
                }

                let metric = sum_inf(neigh.link_cost, neigh_route.metric);

                // if the table has the route
                if let Some(table_route) = self.routes.get_mut(src) {
                    // update route table if the entry is better
                    if let Some(new_fd) = Self::is_feasible(table_route, neigh_route, metric) {
                        // we have a better route!
                        table_route.metric = metric;
                        table_route.source = neigh_route.source.clone();
                        table_route.fd = new_fd;
                        table_route.link = link.clone();
                        table_route.next_hop = neigh.addr.clone();
                        table_route.retracted = false;
                    } else {
                        let nh = &table_route.next_hop;
                        let fd = table_route.fd;
                        // this is a selected route, we should update this regardless.
                        if *nh == neigh.addr {
                            // update route metric
                            if metric > fd {
                                // infeasible route, we should retract this
                                table_route.metric = INF;
                                if !table_route.retracted{
                                    retractions.push(table_route.source.clone());
                                }
                                table_route.retracted = true;
                            } else {
                                // same or better route
                                table_route.metric = metric;
                                table_route.fd = metric;
                                table_route.retracted = false;
                            }
                        }
                    }
                } else if metric != INF {
                    // create the new route, if it is valid
                    let n_route = Route{
                        source: neigh_route.source.clone(),
                        metric,
                        fd: metric,
                        link: link.clone(),
                        next_hop: neigh.addr.clone(),
                        retracted: false,
                    };
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
        let tmp_seqno = self.broadcast_route_for.drain().collect::<Vec<T::NodeAddress>>();
        for source in tmp_seqno {
            if let Some(pkt) = self.create_seqno_packet(&source) {
                self.write_broadcast_packet(&pkt);
            }
        }
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
    ) -> Result<(), RoutingError<T>> {
        if !self.mac_sys.validate(data, neigh) {
            return Err(MACValidationFail {
                link: link.clone()
            });
        }

        // if exists, contains the address we should broadcast
        // let mut broadcast_seqno_for: Option<T::NodeAddress> = None;

        match data.data() {
            Packet::UrgentRouteUpdate(route) => {
                // println!("[dbg] {} got packet {} from {}", json!(self.address), json!(data), json!(neigh));
                match self.handle_neighbour_route_update(route, link, neigh)? {
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
                    self.handle_neighbour_route_update(route, link, neigh)?; // we dont need to worry about seqno updates and retractions
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
                        let original = self.seqno;
                        increment(&mut self.seqno);
                        if seqno_less_than(self.seqno, *seqno) && T::TRUST_RESYNC_SEQNO{
                            self.seqno = *seqno; // did our node go to sleep? we have less seqno than what others are requesting.
                            // MAKE SURE TO ENABLE MAC IN PROD
                            self.warn(DesynchronizedSeqno {
                                old_seqno: original,
                                new_seqno: self.seqno
                            });
                        }
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
        Ok(())
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
    ) -> Result<UpdateAction, RoutingError<T>> {
        let Source { addr: src, seqno } = update.source.data();

        if *src == self.address{
            // just ignore this
            return Ok(NoAction);
        }

        // validate update
        if !self.mac_sys.validate(&update.source, src) {
            return Err(MACValidationFail {
                link: link.clone()
            });
        }

        let mut action = NoAction;
        let stored_seqno = self.get_seqno_for(src);
        if let Some(d_seqno) = stored_seqno {
            if seqno_less_than(*seqno, d_seqno) {
                return Ok(NoAction); // our neighbour is probably out of date. seqno cannot decrease
            } else if seqno_less_than(d_seqno, *seqno) {
                action = SeqnoUpdate; // our neighbour has a higher seqno than us!
            }
        }
        
        // check if this route is the currently selected route
        let mut selected = false;
        if let Some(route) = self.routes.get(src){
            selected = route.next_hop == *neigh;
        }

        if let Some(neighbour) = self.links.get_mut(link) {
            // update the value
            if let Some(table_route) = neighbour.routes.get_mut(src){
                table_route.source = update.source.clone();
                if update.metric == INF{
                    // handle retraction
                    
                    // make sure we don't re-retract an already retracted route
                    if !table_route.retracted {
                        if action == NoAction && selected{
                            // broadcast this retraction, since we are advertising it
                            action = Retraction;
                        }
                        table_route.metric = INF;
                        table_route.retracted = true
                    }
                }
                else{
                    table_route.metric = update.metric;
                }
            }
            else if update.metric != INF || selected {
                // we add the route if it is not INF, or if it is selected
                let route = ExternalRoute {
                source: update.source.clone(),
                    metric: update.metric,
                    retracted: update.metric == INF
                };
                neighbour.routes.insert(src.clone(), route);
            }
        }
        Ok(action)
    }
}

#[derive(Default)]
pub struct NoMACSystem {

}

#[derive(Educe)]
#[educe(Clone(bound()))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize), serde(bound = ""))]
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