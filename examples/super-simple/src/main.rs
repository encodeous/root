use std::collections::HashMap;
use root::concepts::neighbour::Neighbour;
use root::concepts::packet::OutboundPacket;
use root::concepts::route::Route;
use root::framework::RoutingSystem;
use root::router::{NoMACSystem, Router};

struct SimpleExample {} // just a type to inform root of your network parameters
impl RoutingSystem for SimpleExample{
    type NodeAddress = String; // our nodes have string names
    type Link = i32;
    type MACSystem = NoMACSystem; // we won't use MAC for this example
}

fn main() {
    // we have the following connection: bob <-> eve <-> alice

    let mut nodes = HashMap::new();

    let mut bob = Router::<SimpleExample>::new("bob".to_string());
    bob.links.insert(1, Neighbour::new("eve".to_string()));
    nodes.insert("bob", bob);

    let mut eve = Router::<SimpleExample>::new("eve".to_string());
    eve.links.insert(1, Neighbour::new("bob".to_string()));
    eve.links.insert(2, Neighbour::new("alice".to_string()));
    nodes.insert("eve", eve);

    let mut alice = Router::<SimpleExample>::new("alice".to_string());
    alice.links.insert(2, Neighbour::new("eve".to_string()));
    nodes.insert("alice", alice);

    // lets simulate routing!

    for step in 0..3 {
        // collect all of our packets, if any
        let packets: Vec<OutboundPacket<SimpleExample>> = nodes.iter_mut().flat_map(|(_id, node)| node.outbound_packets.drain(..)).collect();

        for OutboundPacket{link, dest, packet} in packets{
            // deliver the routing packet. in this simple example, the link isn't really used
            if let Some(node) = nodes.get_mut(dest.as_str()){
                node.handle_packet(&packet, &link, &dest).expect("Failed to handle packet");
            }
        }

        for node in nodes.values_mut(){
            node.full_update(); // performs route table calculations, and writes routing updates into outbound_packets
        }

        // lets observe bob's route table:
        println!("Bob's routes in step {step}:");
        for (neigh, Route::<SimpleExample>{ metric, next_hop, .. }) in &nodes["bob"].routes{
            println!(" - {neigh}: metric: {metric}, next_hop: {next_hop}")
        }
    }

    // OUTPUT:
    // Bob's routes in step 0:
    // Bob's routes in step 1:
    // - eve: metric: 1, next_hop: eve
    // Bob's routes in step 2:
    // - eve: metric: 1, next_hop: eve
    //     - alice: metric: 2, next_hop: eve
}
