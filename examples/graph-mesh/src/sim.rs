use std::collections::BTreeMap;
use root::concepts::packet::Packet;
use crate::graph_parse::State;
use crate::{DummyMAC, GraphSystem};

pub fn tick_state(state: &mut State){
    let broadcast = *state.config.entry("broadcast_updates".to_string()).or_insert(false);
    let update_routes = *state.config.entry("update_routes".to_string()).or_insert(true);
    let refresh_interfaces = *state.config.entry("refresh_interfaces".to_string()).or_insert(true);

    let mut out_packets: BTreeMap<u8, Vec<DummyMAC<Packet<GraphSystem>>>> = BTreeMap::new();

    // handle packets
    for node in state.nodes.iter_mut() {
        if let Some(packets) = state.packets.get(&node.router.address){
            for (packet, addr) in packets{
                let n_pkt = node.router.handle_packet(packet, &(1u8), addr);
                if let Some(pkt) = n_pkt{
                    let values = out_packets.entry(*addr).or_default();
                    values.push(pkt);
                }
            }
        }
    }
    state.packets.clear();

    for node in state.nodes.iter_mut() {
        if refresh_interfaces {
            node.router.refresh_interfaces()
        }
        if update_routes{
            node.router.update_routes();
        }

        for req in &state.seq_requests{
            if *req == node.router.address{
                let packets = node.router.solve_starvation();
                for (id, itf) in &node.router.interfaces{
                    for (addr, _) in &itf.neighbours{
                        let values = state.packets.entry(*addr).or_default();

                        for packet in &packets{
                            values.push((packet.clone(), node.router.address));
                        }
                    }
                }
            }
        }

        if broadcast {
            let packet = node.router.batch_update();
            for (id, itf) in &node.router.interfaces{
                for (addr, _) in &itf.neighbours{
                    let values = state.packets.entry(*addr).or_default();

                    // this is unrealistic, in a real network we cannot assume the id of the interfaces on the receiving end are the same as the sender's interface id
                    values.push((packet.clone(), node.router.address));
                }
            }
        }
        
        // push all outgoing packets from handling packets
        if let Some(packets) = out_packets.get(&node.router.address){
            for (id, itf) in &node.router.interfaces{
                for (addr, _) in &itf.neighbours{
                    let values = state.packets.entry(*addr).or_default();

                    for packet in packets{
                        values.push((packet.clone(), node.router.address));
                    }
                }
            }
        }
    }
}