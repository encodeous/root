use crate::concepts::interface::{Interface, NetworkInterface};
use crate::concepts::neighbour::Neighbour;
use crate::concepts::packet::{OutboundPacket, Packet, RouteUpdate};
use crate::concepts::route::{Route, Source};
use crate::framework::{MACSystem, RoutingSystem};
use crate::router::UpdateAction::{NoAction, Retraction, SeqnoUpdate};
use crate::util::{increment_by, seqno_less_than, sum_inf};
use log::{error, trace, warn};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::cmp::min;
use std::collections::{HashMap, HashSet};
use std::ops::DerefMut;
use std::time::Instant;

pub const INF: u16 = 0xFFFF;

pub struct Router<T: RoutingSystem> {
    pub interfaces: HashMap<T::InterfaceId, Interface<T>>,
    /// Source, Route
    pub routes: HashMap<T::NodeAddress, Route<T>>,
    pub address: T::NodeAddress,
    pub seqno_requests: HashMap<T::NodeAddress, u16>,
    pub broadcast_route_for: HashSet<T::NodeAddress>,
    pub outbound_packets: Vec<OutboundPacket<T>>,
}

#[derive(Eq, PartialEq)]
enum UpdateAction {
    SeqnoUpdate,
    Retraction,
    NoAction,
}

impl<T: RoutingSystem> Router<T> {
    pub fn new(address: T::NodeAddress) -> Self {
        Self {
            interfaces: HashMap::new(),
            routes: HashMap::new(),
            address,
            seqno_requests: HashMap::new(),
            broadcast_route_for: HashSet::new(),
            outbound_packets: Vec::new(),
        }
    }

    // region Interface
    pub fn add_interface(&mut self, interface: Box<dyn NetworkInterface<T>>) {
        for (id, itf) in &mut self.interfaces {
            if *id == interface.id() {
                // interface exists
                return;
            }
        }
        let n_itf = Interface {
            net_if: interface,
            neighbours: Default::default(),
        };
        self.interfaces.insert(n_itf.net_if.id(), n_itf);
    }
    /// writes a packet to the outbound packet queue for all neighbours
    pub fn write_broadcast_packet(&mut self, packet: &T::MAC<Packet<T>>) {
        // send to all neighbours
        for (itf_id, itf) in &self.interfaces {
            for (n_addr, neigh) in &itf.neighbours {
                self.outbound_packets.push(OutboundPacket {
                    addr_phy: neigh.addr_phy.clone(),
                    itf: itf_id.clone(),
                    packet: packet.clone(),
                });
            }
        }
    }
    pub fn remove_interface(&mut self, id: T::InterfaceId) {
        self.interfaces.retain(|itf_id, _| *itf_id != id);
    }

    /// only call once
    pub fn init(&mut self) {
        // create a self route
        self.routes
            .insert(self.address.clone(), self.make_self_route_for_seqno(0));
    }

    fn make_self_route_for_seqno(&self, seqno: u16) -> Route<T> {
        Route {
            itf: None,
            fd: None,
            metric: 0,
            next_hop: None,
            source: T::MAC::sign(
                Source {
                    addr: self.address.clone(),
                    seqno,
                },
                self,
            ),
        }
    }

    /// Queries the physical network interfaces for neighbours
    pub fn refresh_interfaces(&mut self) {
        for (id, itf) in &mut self.interfaces {
            // pull data from network interfaces
            // itf.neighbours.retain(|_, v| itf.net_if.get_cost(&v.addr_phy) != INF); -- prob shouldnt do this
            for (phy, addr) in itf.net_if.get_neighbours() {
                if let Some(val) = itf.neighbours.get_mut(&addr) {
                    if val.addr_phy == phy {
                        continue; // ok, the net address didn't change
                    }
                    // the address changed!!!
                    trace!(
                        "Network addr of neighbour {} changed from {} to {}",
                        json!(addr),
                        json!(val.addr_phy),
                        json!(phy)
                    );
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

    pub fn solve_starvation(&mut self) {
        let mut packets = Vec::new();
        for (addr, route) in &self.routes {
            // check if starved
            if route.metric == INF {
                // starved
                let cur_seqno = self.get_seqno_for(addr);
                if let Some(seqno) = cur_seqno {
                    let nseqno = increment_by(seqno, 1);
                    packets.push(T::MAC::sign(
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
            if metric < fd
                || seqno_less_than(s, n)
                || (metric == fd && selected_route.metric == INF)
            // we want to restore the route if it was down
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
        for (addr, route) in &mut self.routes {
            if let Some(nh) = &route.next_hop {
                if let Some(itf) = &route.itf {
                    // this should always be true if the next hop exists
                    if let Some(x) = self.interfaces.get(itf) {
                        if !x.neighbours.contains_key(nh) {
                            // disconnected route, should retract
                            route.metric = INF;
                            retractions.push(route.source.clone());
                        }
                    }
                }
            }
        }
        for (id, itf) in &mut self.interfaces {
            for (n_addr, neigh) in &itf.neighbours {
                let cost = itf.net_if.get_cost(&neigh.addr_phy);

                for (src, neigh_route) in &neigh.routes {
                    let metric = sum_inf(cost, neigh_route.metric);
                    let entry = self.routes.get_mut(src);

                    // if the table has the route
                    if let Some(table_route) = entry {
                        // update route table if the entry is better
                        if let Some(new_fd) = Self::is_feasible(table_route, neigh_route, metric) {
                            // we have a better route!
                            // or a route has been retracted
                            table_route.next_hop = Some(neigh.addr.clone());
                            table_route.metric = metric;
                            table_route.source = neigh_route.source.clone();
                            table_route.fd = Some(new_fd);
                            table_route.itf = Some(id.clone());
                        } else if let Some(nh) = &table_route.next_hop {
                            // this is a selected route, we should update this regardless.
                            if nh == n_addr {
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
                        n_route.itf = Some(id.clone());

                        self.routes.insert(src.clone(), n_route);
                    }
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
        for (addr, route) in &self.routes {
            vec.push(RouteUpdate {
                source: route.source.clone(),
                metric: route.metric,
            })
        }
        self.write_broadcast_packet(&T::MAC::sign(
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
    fn create_seqno_packet(&self, addr: &T::NodeAddress) -> Option<T::MAC<Packet<T>>> {
        if let Some(route) = self.routes.get(addr) {
            return Some(T::MAC::sign(
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
    fn write_retraction_for(&mut self, source: T::MAC<Source<T>>) {
        self.write_broadcast_packet(&T::MAC::sign(
            Packet::UrgentRouteUpdate(RouteUpdate {
                source,
                metric: INF,
            }),
            self,
        ))
    }

    /// handle a single packet. if there is a response, it should be broadcast to ALL neighbours
    pub fn handle_packet(
        &mut self,
        data: &T::MAC<Packet<T>>,
        itf: &T::InterfaceId,
        neigh: &T::NodeAddress,
    ) {
        if !data.validate(neigh) {
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
                match self.handle_neighbour_route_update(route, itf, neigh) {
                    SeqnoUpdate => {
                        // lets rebroadcast this change, our seqno has increased!
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
                    self.handle_neighbour_route_update(route, itf, neigh);
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
                        // println!("[dbg] got matched seqno req req_seqno={}, node={}", json!(seqno), json!(self.address));
                        let new_source = T::MAC::sign(
                            Source {
                                addr: source.clone(),
                                seqno: increment_by(cur_seqno, 1),
                            },
                            self,
                        );

                        // println!("[dbg] cur_seqno={seqno}, new_seqno={}", new_source.data().seqno);

                        self.routes
                            .entry(source.clone())
                            .and_modify(|route| route.source = new_source);

                        self.broadcast_route_for.insert(source.clone());
                    } else {
                        let req_seqno = self.seqno_requests.entry(source.clone()).or_insert(0);
                        // prevent duplication and infinite amplification... :skull:
                        if seqno_less_than(*req_seqno, *seqno) {
                            // println!("[dbg] re-requesting seqno src={}, cur_seqno={cur_seqno}, node={}", json!(source), json!(self.address));
                            // sadge, we need to request for seqno too
                            *req_seqno = *seqno; // make sure we dont ask for this seqno again
                            self.write_broadcast_packet(&T::MAC::sign(
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
        itf: &T::InterfaceId,
        neigh: &T::NodeAddress,
    ) -> UpdateAction {
        let Source { addr, seqno } = update.source.data();

        // validate update
        if !update.source.validate(addr) {
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
                action = SeqnoUpdate;
            }
        }

        if let Some(neighbour) = self.get_neighbour_mut(itf, neigh) {
            // update the value
            if let Some(entry) = neighbour.routes.get_mut(addr) {
                entry.source = update.source.clone();
                entry.metric = update.metric;
            } else {
                let route = Route {
                    source: update.source.clone(),
                    metric: update.metric,
                    itf: None,
                    fd: None,
                    next_hop: None,
                };
                neighbour.routes.insert(addr.clone(), route);
            }

            if update.metric == INF {
                // this is a retraction!
                if action != NoAction {
                    error!("Unexpected state: A seqno increase should not have a metric of INF!")
                }
                action = Retraction;
            }
        }
        action
    }

    pub fn get_neighbour(
        &self,
        itf: &T::InterfaceId,
        neigh: &T::NodeAddress,
    ) -> Option<&Neighbour<T>> {
        if let Some(interface) = self.interfaces.get(itf) {
            if let Some(neighbour) = interface.neighbours.get(neigh) {
                return Some(neighbour);
            }
        }
        None
    }
    pub fn get_neighbour_mut(
        &mut self,
        itf: &T::InterfaceId,
        neigh: &T::NodeAddress,
    ) -> Option<&mut Neighbour<T>> {
        if let Some(interface) = self.interfaces.get_mut(itf) {
            if let Some(neighbour) = interface.neighbours.get_mut(neigh) {
                return Some(neighbour);
            }
        }
        None
    }
}
