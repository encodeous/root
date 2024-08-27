use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use root::concepts::neighbour::Neighbour;
use root::concepts::packet::Packet;
use root::framework::RoutingSystem;
use root::router::{DummyMAC, NoMACSystem, Router};

#[derive(Serialize, Deserialize)]
pub struct VirtualSystem{
    pub routers: Vec<Router<VirtualSystem>>,
    pub packets: BTreeMap<String, Vec<(DummyMAC<Packet<VirtualSystem>>, i32)>>,
}

impl VirtualSystem{
    pub fn create(nodes: &[&str], links: &[(i32, &str, &str, u16)]) -> VirtualSystem{
        let routers: Vec<Router<VirtualSystem>> = nodes.iter().map(|id|{
            let mut router = Router::new(id.to_string());
            for (lid, a, b, metric) in links{
                if a == id || b == id {
                    let nid = {
                        if a == id {b} else {a}
                    };
                    router.links.insert(*lid, Neighbour{
                        link: *lid,
                        addr: nid.to_string(),
                        routes: Default::default(),
                        link_cost: *metric,
                    });
                }
            }
            router
        }).collect();
        VirtualSystem{
            routers,
            packets: Default::default()
        }
    }

    pub fn update_edge(&mut self, edge_id: i32, metric: u16){
        for router in &mut self.routers{
            router.links.entry(edge_id).and_modify(|edge| {
                edge.link_cost = metric
            });
        }
    }
    
    pub fn get_node(&mut self, node: &str) -> &mut Router<Self>{
        self.routers.iter_mut().find(|r| r.address == node).unwrap()
    }
    
    pub fn get_next_hop(&self, cur: &str, src: &str) -> String{
        let router = self.routers.iter().find(|r|r.address == cur).unwrap_or_else(|| panic!("No node {cur} found"));
        router.routes.get(src).unwrap_or_else(|| panic!("No route found to {src}")).next_hop.to_string()
    }

    pub fn get_metric_to(&self, cur: &str, src: &str) -> u16{
        let router = self.routers.iter().find(|r|r.address == cur).unwrap_or_else(|| panic!("No node {cur} found"));
        router.routes.get(src).unwrap_or_else(|| panic!("No route found to {src}")).metric
    }

    pub fn get_seqno_to(&self, cur: &str, src: &str) -> u16{
        let router = self.routers.iter().find(|r|r.address == cur).unwrap_or_else(|| panic!("No node {cur} found"));
        router.routes.get(src).unwrap_or_else(|| panic!("No route found to {src}")).source.data.seqno
    }

    pub fn flush_packets(&mut self){
        for router in &mut self.routers{
            for packet in router.outbound_packets.drain(..){
                let link_packets = self.packets.entry(packet.dest).or_default();
                link_packets.push((packet.packet, packet.link));
            }
        }
    }
    
    pub fn tick(&mut self){
        for (node, packets) in &mut self.packets{
            if let Some(router) = self.routers.iter_mut().find(|x|x.address == *node){
                for (packet, link) in packets{
                    if let Some(neigh_addr) = router.links.get(link).map(|x| x.addr.clone()){
                        router.handle_packet(packet, link, &neigh_addr);
                    }
                }
            }
        }
        self.packets.clear();
        for router in &mut self.routers{
            router.full_update();
        }
        self.flush_packets()
    }

    pub fn tick_n(&mut self, times: i32){
        for _ in 0..times{
            self.tick();
        }
    }

    pub fn freeze(&mut self) -> String{
        serde_json::to_string(&self).unwrap()
    }

    pub fn restore(state: String) -> VirtualSystem{
        serde_json::from_str(&state).unwrap()
    }
}

impl RoutingSystem for VirtualSystem{
    type NodeAddress = String;
    type Link = i32;
    type MACSystem = NoMACSystem;
}