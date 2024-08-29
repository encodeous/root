use std::cmp::max;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddrV4};
use std::process::exit;
use std::str::FromStr;
use std::time::{Duration, Instant};
use anyhow::{anyhow, Context};
use crossbeam_channel::{Receiver, unbounded};
use log::{debug, error, info, trace, warn};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{sleep, timeout};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use root::concepts::neighbour::Neighbour;
use root::framework::RoutingSystem;
use root::router::{DummyMAC, INF};
use crate::link::NetLink;
use crate::packet::{NetPacket, RoutedPacket};
use crate::packet::NetPacket::{LinkRequest, Ping, Pong, TraceRoute};
use crate::routing::IPV4System;
use crate::state::{LinkHealth, MainLoopEvent, MessageQueue, OperatingState, PersistentState, QueuedPacket};
use crate::state::MainLoopEvent::{DispatchPingLink, InboundPacket, NoEvent, PingResultFailed, RoutePacket, Shutdown, TimerPingUpdate, TimerRouteUpdate};

pub fn start_router(ps: PersistentState, os: OperatingState) -> MessageQueue{
    let (mtx, mrx) = unbounded();
    let (otx, orx) = unbounded();
    let ct = CancellationToken::new();
    let mq = MessageQueue{
        main: mtx,
        outbound: otx,
        cancellation_token: ct
    };
    let tmq = mq.clone();
    tokio::task::spawn_blocking(||{
        packet_sender(tmq, orx).context("Packet Sender Thread Failed: ").unwrap();
    });
    let tmq = mq.clone();
    tokio::task::spawn_blocking(||{
        main_loop(ps, os, tmq, mrx).context("Main Thread Failed: ").unwrap();
    });
    tokio::spawn(server(mq.clone()));
    let tmq = mq.clone();
    // ping neighbours
    tokio::spawn(async move {
        while !tmq.cancellation_token.is_cancelled(){
            tmq.main.send(TimerPingUpdate).unwrap();
            sleep(Duration::from_secs(5)).await;
        }
    });
    let tmq = mq.clone();
    // broadcast routes
    tokio::spawn(async move {
        while !tmq.cancellation_token.is_cancelled(){
            tmq.main.send(TimerRouteUpdate).unwrap();
            sleep(Duration::from_secs(10)).await;
        }
    });
    mq
}

// PACKET SENDER THREAD
fn packet_sender(
    mq: MessageQueue,
    mr: Receiver<QueuedPacket>,
) -> anyhow::Result<()> {
    let mut connections = HashMap::<Ipv4Addr, tokio::sync::mpsc::Sender<QueuedPacket>>::new();
    while !mq.cancellation_token.is_cancelled(){
        let packet = mr.recv().unwrap();

        if let Entry::Vacant(e) = connections.entry(packet.to) {
            let (tx, rx) = tokio::sync::mpsc::channel(1000);
            e.insert(tx);
            tokio::spawn(link_io(mq.clone(), rx, packet.to));
        }

        let mut remove = false;

        let to = packet.to;

        if let Some(tx) = connections.get(&packet.to){
            if tx.capacity() > 0 && tx.blocking_send(packet).is_err() {
                remove = true;
            }
        }

        if remove{
            connections.remove(&to);
        }
    }
    Ok(())
}

// LINK THREAD(s)

async fn link_io(
    mq: MessageQueue,
    mut mr: tokio::sync::mpsc::Receiver<QueuedPacket>,
    dst: Ipv4Addr
) -> anyhow::Result<()>{
    while !mq.cancellation_token.is_cancelled(){
        let res = timeout(Duration::from_millis(5000), TcpStream::connect(SocketAddrV4::new(dst, 9988))).await;

        let mut status: anyhow::Result<()> = Ok(());
        let mut fail = NoEvent;

        if let Ok(Ok(mut stream)) = res{
            debug!("Connected to {dst}");
            let mut bytes: Vec<u8> = Vec::new();
            while !mq.cancellation_token.is_cancelled(){
                let write = async {
                    for _ in 0..max(mr.len(), 1){
                        let packet = mr.recv().await.ok_or(anyhow!("Stream Ended"))?;
                        fail = packet.failure_event;
                        trace!("Writing packet: {} to {dst}", json!(packet.packet));
                        serde_json::to_writer(&mut bytes, &packet.packet)?;
                        stream.write_u32(bytes.len() as u32).await?;
                        stream.write_all(&bytes).await?;
                        bytes.clear();
                    }

                    stream.flush().await?;

                    Ok(())
                };

                if let Err(x) = write.await{
                    status = Err(x);
                    break;
                }
            }
            return Ok(())
        }
        // failure

        if let Err(x) = status{
            if let NoEvent = fail{}
            else{
                mq.main.send(fail)?;
            }
            debug!("Error occurred while trying to write packet to {dst}: {x:?}");
        }

        let wakeup = Instant::now() + Duration::from_secs(10);
        
        while Instant::now() < wakeup{
            if let Ok(Some(packet)) = timeout(Duration::from_millis(100), mr.recv()).await{
                if let NoEvent = packet.failure_event{}
                else{
                    mq.main.send(packet.failure_event)?;
                }
            }
        }
    }

    Ok(())
}

// SERVER THREAD

async fn server(mq: MessageQueue) -> anyhow::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:9988").await?;
    while !mq.cancellation_token.is_cancelled(){
        let (mut sock, addr) = listener.accept().await?;

        debug!("Got connection from {addr}");
        if let IpAddr::V4(addr) = addr.ip() {
            let tmq = mq.clone();
            tokio::spawn(async move {
                debug!("Started worker task for incoming connection.");
                let reader = async {
                    let mut bytes: Vec<u8> = Vec::new();
                    while !tmq.cancellation_token.is_cancelled() {
                        let len = sock.read_u32().await? as usize;
                        if len > 60000 {
                            break;
                        }
                        bytes.resize(len, 0);
                        sock.read_exact(&mut bytes[..len]).await?;
                        let packet: NetPacket = serde_json::from_slice(&bytes[..len])?;
                        trace!("Got packet {} from {addr}", json!(packet));
                        tmq.main.send(InboundPacket {
                            address: addr,
                            packet
                        })?;
                    }
                    anyhow::Result::<()>::Ok(())
                };

                if let Err(x) = reader.await{
                    // ignored
                    debug!("Error in main thread: {x}");
                }
            });
        }
    }
    Ok(())
}

// MAIN THREAD

fn main_loop(
    mut ps: PersistentState,
    mut os: OperatingState,
    mqs: MessageQueue,
    mqr: Receiver<MainLoopEvent>
) -> anyhow::Result<()>{
    while !mqs.cancellation_token.is_cancelled() {
        let event = mqr.recv()?;
        trace!("Main Loop Event: {}", json!(event));
        match event {
            MainLoopEvent::InboundPacket { address, packet } => {
                handle_packet(&mut ps, &mut os, mqs.clone(), packet, address)?
            }
            RoutePacket { to, from, packet } => {
                route_packet(&mut ps, &mut os, mqs.clone(), packet, to, from)?;
            }
            MainLoopEvent::DispatchCommand(cmd) => {
                if let Err(err) = handle_command(&mut ps, &mut os, cmd, mqs.clone()){
                    error!("Error while handling command: {err}");
                }
            }
            MainLoopEvent::TimerRouteUpdate => {
                ps.router.full_update();
                write_routing_packets(&mut ps, &mut os, mqs.clone())?;
            }
            MainLoopEvent::TimerPingUpdate => {
                for lid in ps.links.keys(){
                    mqs.main.send(DispatchPingLink {link_id: *lid})?;
                }
            }
            Shutdown => {
                mqs.cancellation_token.cancel()
            }
            MainLoopEvent::DispatchPingLink { link_id } => {
                let entry = os.health.entry(link_id).or_insert(
                    LinkHealth {
                        ping: Duration::MAX,
                        ping_start: Instant::now(),
                        last_ping: Instant::now(),
                    }
                );
                entry.ping_start = Instant::now();

                if let Some(netlink) = ps.links.get(&link_id){
                    mqs.outbound.send(
                        QueuedPacket{
                            to: netlink.neigh_addr,
                            packet: Ping(link_id),
                            failure_event: PingResultFailed {
                                link_id
                            }
                        }
                    )?;
                }
            }
            PingResultFailed { link_id } => {
                if let Some(netlink) = ps.links.get(&link_id){
                    debug!("Error while pinging {}", netlink.link);
                    os.health.entry(link_id).and_modify(|entry|{
                        entry.ping = Duration::MAX;
                    });
                    update_link_health(&mut ps, link_id, Duration::MAX)?;
                }
            }
            MainLoopEvent::NoEvent => {
                // do nothing
            }
        }
        
        for warn in ps.router.warnings.drain(..){
            warn!("{warn:?}");
        }
    }

    info!("The router has shutdown, saving state...");

    let content = {
        serde_json::to_vec(&ps)?
    };
    fs::write("./config.json", content)?;
    debug!("Saved State");

    exit(0);
    Ok(())
}

fn handle_packet(
    ps: &mut PersistentState,
    os: &mut OperatingState,
    mq: MessageQueue,
    pkt: NetPacket,
    from: Ipv4Addr,
) -> anyhow::Result<()> {
    debug!("Handling packet {}", json!(pkt));

    match pkt {
        Ping(id) => {
            if let Some(link) = ps.links.get(&id) {
                debug!("Ping received from {} nid: {}", link.neigh_addr, link.neigh_node);
                mq.outbound.send(
                    QueuedPacket{
                        to: link.neigh_addr,
                        packet: Pong(id),
                        failure_event: NoEvent
                    }
                )?;
            }
        }
        Pong(id) => {
            if let Some(link) = ps.links.get(&id) {
                debug!("Pong received from {}", link.neigh_addr);
                if let Some(health) = os.health.get_mut(&id){
                    health.last_ping = Instant::now();
                    health.ping = (Instant::now() - health.ping_start) / 2;
                    update_link_health(ps, id, health.ping)?;
                }
            }
        }
        NetPacket::Routing { link_id, data } => {
            if let Some(link) = ps.links.get(&link_id) {
                if os.log_routing {
                    info!("RP From: {}, {}, via {}", link.neigh_node, json!(data), link.link);
                }
                let n_nid = link.neigh_node.clone();
                ps.router.handle_packet(&DummyMAC::from(data), &link_id, &n_nid)?;
                ps.router.update();
                write_routing_packets(ps, os, mq)?;
            }
        }
        LinkRequest { link_id, from: from_node } => {
            info!("LINKING REQUEST: {link_id} from {from}.\nType \"alink {from_node}\" to accept.");
            os.link_requests.insert(from_node.clone(), NetLink {
                link: link_id,
                neigh_addr: from.clone(),
                neigh_node: from_node,
            });
        }
        NetPacket::LinkResponse { link_id, node_id } => {
            info!("LINKING SUCCESS: {node_id} has accepted the link {link_id}.");
            if let Some(mut net_link) = os.unlinked.remove(&link_id) {
                net_link.neigh_node = node_id.clone();
                ps.router.links.insert(
                    link_id,
                    Neighbour {
                        addr: node_id.clone(),
                        link: link_id,
                        link_cost: INF,
                        routes: HashMap::new(),
                    },
                );
                ps.links.insert(link_id, net_link);
            }
        }
        NetPacket::Deliver { dst_id, sender_id, data } => {
            if dst_id == ps.router.address {
                handle_routed_packet(ps, os, mq, data, sender_id)?;
            } else {
                // do routing
                route_packet(ps, os, mq, data, dst_id, sender_id)?;
            }
        }
        NetPacket::TraceRoute { dst_id, sender_id, mut path } => {
            path.push(ps.router.address.clone());
            if dst_id == ps.router.address {
                mq.main.send(RoutePacket {
                    packet: RoutedPacket::TracedRoute {
                        path
                    },
                    from: ps.router.address.clone(),
                    to: sender_id
                })?;
            } else {
                // do routing
                if let Some(route) = ps.router.routes.get(&dst_id) {
                    if os.log_routing {
                        info!("TRT sender: {}, dst: {}, nh: {}", sender_id, dst_id, route.next_hop);
                    }
                    // forward packet
                    if let Some(netlink) = ps.links.get(&route.link) {
                        mq.outbound.send(
                            QueuedPacket{
                                to: netlink.neigh_addr.clone(),
                                packet: TraceRoute {
                                    dst_id,
                                    sender_id,
                                    path,
                                },
                                failure_event: NoEvent
                            }
                        )?;
                    }
                }
            }
        }
    }

    Ok(())
}

fn write_routing_packets(ps: &mut PersistentState,
                               os: &mut OperatingState,
                               mq: MessageQueue) -> anyhow::Result<()> {
    for pkt in ps.router.outbound_packets.drain(..){
        if let Some(netlink) = ps.links.get(&pkt.link){
            mq.outbound.send(
                QueuedPacket{
                    to: netlink.neigh_addr,
                    packet: NetPacket::Routing {
                        link_id: pkt.link,
                        data: pkt.packet.data.clone(),
                    },
                    failure_event: NoEvent
                }
            )?;
        }
    }
    Ok(())
}

fn update_link_health(
    ps: &mut PersistentState,
    link: Uuid,
    new_ping: Duration,
) -> anyhow::Result<()> {
    if let Some(neigh) = ps.router.links.get_mut(&link) {
        neigh.link_cost = {
            if new_ping == Duration::MAX {
                INF
            } else {
                max(new_ping.as_millis() as u16, 1)
            }
        }
    }
    ps.router.update();
    Ok(())
}


fn route_packet(
    ps: &mut PersistentState,
    os: &mut OperatingState,
    mq: MessageQueue,
    data: RoutedPacket,
    dst_id: <IPV4System as RoutingSystem>::NodeAddress,
    sender_id: <IPV4System as RoutingSystem>::NodeAddress
) -> anyhow::Result<()> {
    if dst_id == ps.router.address {
        handle_routed_packet(ps, os, mq, data, sender_id)?;
    } else {
        // do routing
        if let Some(route) = ps.router.routes.get(&dst_id) {
            if os.log_routing {
                info!("DP sender: {}, dst: {}, nh: {}", sender_id, dst_id, route.next_hop);
            }
            // forward packet
            if let Some(netlink) = ps.links.get(&route.link) {
                mq.outbound.send(
                    QueuedPacket{
                        to: netlink.neigh_addr,
                        packet: NetPacket::Deliver {
                            dst_id,
                            sender_id,
                            data,
                        },
                        failure_event: NoEvent
                    }
                )?;
                return Ok(());
            }
        }
    }
    Ok(())
}

fn handle_routed_packet(
    ps: &mut PersistentState,
    os: &mut OperatingState,
    mq: MessageQueue,
    pkt: RoutedPacket,
    src: <IPV4System as RoutingSystem>::NodeAddress
) -> anyhow::Result<()> {
    trace!("Handling routed packet from {src}: {}", json!(pkt));
    match pkt {
        RoutedPacket::Ping => {
            mq.main.send(RoutePacket {
                to: src,
                from: ps.router.address.clone(),
                packet: RoutedPacket::Pong
            })?;
        }
        RoutedPacket::Pong => {
            if let Some(start) = os.pings.remove(&src) {
                info!("Pong from {src} {:?}", (Instant::now() - start) / 2);
            }
        }
        RoutedPacket::TracedRoute { path } => {
            info!("Traced route from {src}: {}", path.join(" -> "));
        }
        RoutedPacket::Message(msg) => {
            info!("{src}> {msg}");
        }
        RoutedPacket::Undeliverable { to } => {
            warn!("Undeliverable destination: {to}");
        }
    }
    Ok(())
}

fn handle_command(
    ps: &mut PersistentState,
    os: &mut OperatingState,
    cmd: String,
    mq: MessageQueue,
) -> anyhow::Result<()> {
    let split: Vec<&str> = cmd.split_whitespace().collect();
    if split.is_empty() {
        return Ok(());
    }
    match split[0] {
        "help" => {
            info!(r#"Help:
                - help -- shows this page
                - exit -- exits and saves state
                [direct link]
                - ls -- lists all direct links
                - link <ip-address> -- set up a link
                - alink <link-id> -- accepts a link
                - dlink <link-id> -- deletes a link
                [routing]
                - route -- prints whole route table
                - ping <node-name> -- pings node
                - msg <node-name> <message> -- sends a message to a node
                - traceroute/tr <node-name> -- traces a route to a node
                [debug]
                - rpkt -- log routing protocol control packets
                - dpkt -- log routing/forwarded packets
                "#);
        }
        "route" => {
            let mut rtable = vec![];
            info!("Route Table:");
            rtable.push(String::new());
            rtable.push(format!("Self: {}, seq: {}", ps.router.address, ps.router.seqno));
            for (addr, route) in &ps.router.routes {
                rtable.push(
                    format!("{addr} - via: {}, nh: {}, c: {}, seq: {}, fd: {}, ret: {}",
                            route.link,
                            route.next_hop.clone(),
                            route.metric,
                            route.source.data.seqno,
                            route.fd,
                            route.retracted
                    ))
            }
            info!("{}", rtable.join("\n"));
        }
        "rpkt" => {
            os.log_routing = !os.log_routing;
        }
        "dpkt" => {
            os.log_delivery = !os.log_delivery;
        }
        "exit" => {
            mq.main.send(Shutdown)?;
        }
        "ping" => {
            if split.len() != 2 {
                return Err(anyhow!("Expected one argument"));
            }
            let node = split[1];
            os.pings.insert(node.to_string(), Instant::now());
            mq.main.send(RoutePacket {
                to: node.to_string(),
                from: ps.router.address.clone(),
                packet: RoutedPacket::Ping
            })?;
        }
        "traceroute" | "tr" => {
            if split.len() != 2 {
                return Err(anyhow!("Expected one argument"));
            }
            let node = split[1];
            if let Some(nh) = ps.router.routes.get(node) {
                if let Some(netlink) = ps.links.get(&nh.link) {
                    let s_addr = ps.router.address.clone();
                    let naddr = netlink.neigh_addr;
                    mq.outbound.send(
                        QueuedPacket{
                            to: naddr,
                            packet: TraceRoute {
                                path: vec![],
                                dst_id: node.to_string(),
                                sender_id: s_addr,
                            },
                            failure_event: NoEvent
                        }
                    )?;
                }
            }
        }
        "msg" => {
            if split.len() <= 2 {
                return Err(anyhow!("Expected at least two arguments"));
            }
            let node = split[1];
            let msg = split[2..].join(" ");
            mq.main.send(RoutePacket {
                to: node.to_string(),
                from: ps.router.address.clone(),
                packet: RoutedPacket::Message(msg)
            })?;
        }
        "ls" => {
            for (id, net) in &ps.links {
                if let Some(health) = os.health.get(id) {
                    info!("id: {id}, addr: {}, ping: {:?}", net.neigh_addr, health.ping)
                } else {
                    info!("id: {id}, addr: {} UNCONNECTED", net.neigh_addr)
                }
            }
        }
        "link" => {
            if split.len() != 2 {
                return Err(anyhow!("Expected one argument"));
            }
            let ip = Ipv4Addr::from_str(split[1])?;
            let id = Uuid::new_v4();
            info!("Sent linking request {id} to {ip}");
            os.unlinked.insert(id, NetLink {
                link: id,
                neigh_addr: ip,
                neigh_node: "UNKNOWN".to_string(),
            });
            mq.outbound.send(
                QueuedPacket{
                    to: ip,
                    packet: LinkRequest {
                        from: ps.router.address.clone(),
                        link_id: id,
                    },
                    failure_event: NoEvent
                }
            )?;
        }
        "alink" => {
            if split.len() != 2 {
                return Err(anyhow!("Expected one argument"));
            }
            if let Some(netlink) = os.link_requests.remove(split[1]) {
                ps.router.links.insert(
                    netlink.link,
                    Neighbour {
                        addr: netlink.neigh_node.clone(),
                        link: netlink.link,
                        link_cost: INF,
                        routes: HashMap::new(),
                    },
                );
                let node_addr = ps.router.address.clone();
                let lid = netlink.link;
                let naddr = netlink.neigh_addr;
                ps.links.insert(netlink.link, netlink);
                mq.outbound.send(
                    QueuedPacket{
                        to: naddr,
                        packet: NetPacket::LinkResponse {
                            link_id: lid,
                            node_id: node_addr,
                        },
                        failure_event: NoEvent
                    }
                )?;
                info!("LINKING SUCCESS");
            } else {
                error!("No matching linking code found!");
            }
        }
        "dlink" => {
            if split.len() != 2 {
                return Err(anyhow!("Expected one argument"));
            }
            let uuid = Uuid::parse_str(split[1])?;
            ps.links.remove(&uuid);
            os.health.remove(&uuid);
        }
        &_ => {
            error!("Unknown command, please try again or type \"help\" for help.")
        }
    }
    Ok(())
}