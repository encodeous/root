use crate::graph_parse::State;

pub fn tick_state(state: &mut State) {
    println!("[tick] New Tick Started");
    let broadcast_routes = *state
        .config
        .entry("broadcast_routes".to_string())
        .or_insert(false);
    let broadcast_seqno = *state
        .config
        .entry("broadcast_seqno".to_string())
        .or_insert(true);
    let update_routes = *state
        .config
        .entry("update_routes".to_string())
        .or_insert(true);

    // handle packets
    for node in state.nodes.iter_mut() {
        if let Some(packets) = state.packets.get(&node.router.address) {
            for (packet, addr) in packets {
                node.router.handle_packet(packet, addr, addr);
            }
        }
    }
    state.packets.clear();

    for node in state.nodes.iter_mut() {
        if update_routes {
            node.router.update_routes();
        }

        for req in &state.seq_requests {
            if *req == node.router.address {
                node.router.solve_starvation();
            }
        }
        
        if broadcast_routes {
            node.router.broadcast_routes();
        }
        if broadcast_seqno {
            node.router.broadcast_seqno_updates();
        }

        // push all outgoing packets from handling packets

        for packet in node.router.outbound_packets.drain(..) {
            // println!("[dbg] OP {} -> {}: {}", node.router.address, packet.addr_phy, json!(packet.packet));
            let values = state.packets.entry(packet.link).or_default();
            values.push((packet.packet, node.router.address))
        }
    }
}
