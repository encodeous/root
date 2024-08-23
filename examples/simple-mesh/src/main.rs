mod link;
mod routing;
mod state;
mod packet;

use std::cmp::max;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, stdin};
use std::net::{IpAddr, Ipv4Addr, SocketAddrV4};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use futures::SinkExt;
use futures::TryStreamExt;
use inquire::{prompt_text};
use log::{debug, error, info, set_boxed_logger, set_max_level, warn};
use serde_json::json;
use simplelog::*;
use tokio::fs;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{Receiver};
use tokio::sync::Mutex;
use tokio::time::sleep;
use root::router::{DummyMAC, INF, Router};
use crate::state::{LinkHealth, OperatingState, PersistentState, SyncState};
use crate::routing::IPV4System;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use tokio_serde::{Framed, SymmetricallyFramed};
use tokio_serde::formats::{Json, SymmetricalJson};
use uuid::{Uuid};
use root::concepts::neighbour::Neighbour;
use root::concepts::packet::OutboundPacket;
use root::framework::RoutingSystem;
use crate::link::NetLink;
use crate::packet::{NetPacket, RoutedPacket};
use crate::packet::NetPacket::{LinkRequest, Ping, Pong, TraceRoute};

async fn save_state(cfg: &PersistentState) -> anyhow::Result<()> {
    fs::write("./config.json", serde_json::to_vec(cfg)?).await?;
    Ok(())
}

async fn setup() -> anyhow::Result<PersistentState> {
    info!("Node Setup (First Time):");
    let mut id;
    loop {
        id = prompt_text("Pick a unique node id (lowercase string, no spaces): ")?;
        if id.bytes().any(|x| !x.is_ascii_lowercase() && x != b'-' && !x.is_ascii_digit()) {
            error!("Try again.")
        } else {
            break;
        }
    }

    info!("Set node id to {id}");

    Ok(PersistentState {
        links: HashMap::new(),
        router: Router::new(id),
    })
}

async fn server(state: Arc<Mutex<SyncState>>) -> anyhow::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:9988").await?;

    loop {
        let (sock, addr) = listener.accept().await?;

        debug!("Got packet from {addr}");

        let len_del = FramedRead::new(sock, LengthDelimitedCodec::new());
        let mut deserialized = SymmetricallyFramed::new(len_del, SymmetricalJson::<NetPacket>::default());
        if let IpAddr::V4(addr) = addr.ip() {
            tokio::spawn({
                let c_state = state.clone();
                let c_addr = addr;
                async move {
                    while let Some(msg) = deserialized.try_next().await.unwrap() {
                        match handle_packet(c_state.clone(), msg, &c_addr).await {
                            Ok(_) => {}
                            Err(err) => {
                                error!("Error occurred while handling packet: {err}");
                            }
                        }
                    }
                }
            });
        }
    }
}

async fn ping(state: Arc<Mutex<SyncState>>, id: Uuid, addr: Ipv4Addr, silent: bool) {
    {
        let mut ss = state.lock().await;
        let entry = ss.os.health.entry(id).or_insert(
            LinkHealth {
                ping: Duration::MAX,
                ping_start: Instant::now(),
                last_ping: Instant::now(),
            }
        );
        entry.ping_start = Instant::now();
    }
    tokio::spawn(async move {
        if let Err(err) = send_packets_wait(addr, vec![Ping(id, silent)]).await {
            debug!("Error while pinging {err}");
            let mut ss = state.lock().await;
            if let Some(x) = ss.os.health.get_mut(&id){
                x.ping = Duration::MAX;
                drop(ss);
                update_link_health(state.clone(), id, Duration::MAX).await.unwrap();
            }
        }
    });
}

async fn ping_updater(state: Arc<Mutex<SyncState>>) -> anyhow::Result<()> {
    loop {
        sleep(Duration::from_millis(5000)).await;
        let ss = state.lock().await;
        let links = ss.ps.links.clone();
        drop(ss);
        for (lid, nlink) in links{
            ping(state.clone(), lid, nlink.neigh_addr, true).await;
        }
    }
}
async fn route_updater(state: Arc<Mutex<SyncState>>) -> anyhow::Result<()> {
    loop {
        sleep(Duration::from_millis(5000)).await;
        let mut ss = state.lock().await;
        ss.ps.router.full_update();
        drop(ss);
        send_outbound(state.clone()).await?;
        let ss = state.lock().await;
        save_state(&ss.ps).await?;
    }
}
async fn packet_sender(mut recv: Receiver<(Ipv4Addr, NetPacket)>) -> anyhow::Result<()> {
    let mut connections: HashMap<Ipv4Addr, Framed<FramedWrite<TcpStream, LengthDelimitedCodec>, NetPacket, NetPacket, Json<NetPacket, NetPacket>>> = HashMap::new();
    let mut next_retry = HashMap::new();
    let mut dirty = HashSet::new();
    let mut buf = Vec::new();
    loop {
        recv.recv_many(&mut buf, 1000).await;
        for (dst, pkt) in buf.drain(..){
            if let std::collections::hash_map::Entry::Vacant(e) = connections.entry(dst) {
                if let Some(time) = next_retry.get(&dst) {
                    if *time > Instant::now(){
                        continue; // dont want to overload anything
                    }
                }
                let res = TcpStream::connect(SocketAddrV4::new(dst, 9988)).await;
                match res {
                    Ok(stream) => {
                        let len_del = FramedWrite::new(stream, LengthDelimitedCodec::new());
                        let symm = SymmetricallyFramed::new(len_del, SymmetricalJson::<NetPacket>::default());
                        e.insert(symm);
                    }
                    Err(err) => {
                        next_retry.insert(dst, Instant::now() + Duration::from_secs(5));
                        debug!("Error while sending packets: {err}");
                        continue;
                    }
                };
            }

            let mut remove = false;

            if let Some(conn) = connections.get_mut(&dst){
                remove = conn.send(pkt).await.is_err();
                dirty.insert(dst);
            }

            if remove{
                connections.remove(&dst);
            }
        }
        for ip in dirty.drain(){
            let mut remove = false;
            if let Some(conn) = connections.get_mut(&ip){
                remove = conn.flush().await.is_err();
            }
            if remove{
                connections.remove(&ip);
            }
        }
    }
}

async fn send_outbound(state: Arc<Mutex<SyncState>>) -> anyhow::Result<()> {
    let mut ss = state.lock().await;
    let out_pkt = ss.ps.router.outbound_packets.drain(..).collect::<Vec<OutboundPacket<IPV4System>>>();
    drop(ss);
    for pkt in out_pkt {
        let ss = state.lock().await;
        if let Some(netaddr) = ss.ps.links.get(&pkt.link){
            let naddr = netaddr.neigh_addr;
            drop(ss);
            send_packet(state.clone(), naddr, NetPacket::Routing {
                link_id: pkt.link,
                data: pkt.packet.data.clone(),
            }).await?;
        }
    }
    Ok(())
}
async fn send_packets_wait(addr: Ipv4Addr, pkts: Vec<NetPacket>) -> anyhow::Result<()> {
    let stream = TcpStream::connect(SocketAddrV4::new(addr, 9988)).await?;
    let len_del = FramedWrite::new(stream, LengthDelimitedCodec::new());
    let mut serialized = SymmetricallyFramed::new(len_del, SymmetricalJson::<NetPacket>::default());
    for pkt in pkts {
        serialized.send(pkt).await?;
    }
    Ok(())
}

async fn send_packet(state: Arc<Mutex<SyncState>>, addr: Ipv4Addr, pkt: NetPacket) -> anyhow::Result<()> {
    let mut ss = state.lock().await;
    if let Some(sender) = &ss.os.packet_queue {
        sender.send((addr, pkt)).await?;
    }
    Ok(())
}

async fn update_link_health(state: Arc<Mutex<SyncState>>, link: Uuid, new_ping: Duration) -> anyhow::Result<()>{
    let mut ss = state.lock().await;
    if let Some(neigh) = ss.ps.router.links.get_mut(&link){
        neigh.link_cost = {
            if new_ping == Duration::MAX{
                INF
            }
            else{
                max(new_ping.as_millis() as u16, 1)
            }
        }
    }
    ss.ps.router.update();
    Ok(())
}

async fn handle_packet(state: Arc<Mutex<SyncState>>, pkt: NetPacket, addr: &Ipv4Addr) -> anyhow::Result<()> {
    let mut ss = state.lock().await;

    match pkt {
        Ping(id, silent) => {
            if let Some(link) = ss.ps.links.get(&id) {
                debug!("Ping received from {} nid: {}", link.neigh_addr, link.neigh_node);
                let naddr = link.neigh_addr;
                drop(ss);
                send_packet(state.clone(), naddr, Pong(id, silent)).await?;
            }
        }
        Pong(id, silent) => {
            if let Some(link) = ss.ps.links.get(&id) {
                if silent {
                    debug!("Pong received from {}", link.neigh_addr);
                } else {
                    info!("Pong received from {}", link.neigh_addr);
                }
                // update link timing
                if let Some(health) = ss.os.health.get_mut(&id) {
                    health.last_ping = Instant::now();
                    health.ping = (Instant::now() - health.ping_start) / 2;
                    let ping = health.ping;
                    drop(ss);
                    update_link_health(state.clone(), id, ping).await?;
                }
            }
        }
        NetPacket::Routing { link_id, data } => {
            if let Some(link) = ss.ps.links.get(&link_id) {
                if ss.os.log_routing {
                    info!("RP From: {}, {}, via {}", link.neigh_node, json!(data), link.link);
                }
                let n_nid = link.neigh_node.clone();
                ss.ps.router.handle_packet(&DummyMAC::from(data), &link_id, &n_nid);
                ss.ps.router.update();
                drop(ss);
                send_outbound(state).await?;
            }
        }
        LinkRequest { link_id, from } => {
            info!("LINKING REQUEST: {link_id} from {from}. Type \"alink {from}\" to accept.");
            ss.os.link_requests.insert(from.clone(), NetLink {
                link: link_id,
                neigh_addr: addr.clone(),
                neigh_node: from,
            });
        }
        NetPacket::LinkResponse { link_id, node_id } => {
            info!("LINKING SUCCESS: {node_id} has accepted the link {link_id}.");
            if let Some(mut net_link) = ss.os.unlinked.remove(&link_id) {
                net_link.neigh_node = node_id.clone();
                ss.ps.router.links.insert(
                    link_id,
                    Neighbour {
                        addr: node_id.clone(),
                        link: link_id,
                        link_cost: INF,
                        routes: HashMap::new(),
                    },
                );
                ss.ps.links.insert(link_id, net_link);
            }
        }
        NetPacket::Deliver { dst_id, sender_id, data } => {
            if dst_id == ss.ps.router.address{
                drop(ss);
                handle_routed_packet(state, data, sender_id).await?;
            }
            else{
                // do routing
                drop(ss);
                route_packet(state, data, dst_id, sender_id, Some(*addr)).await?;
            }
        }
        NetPacket::Undeliverable { dst_id, sender_id } => {
            if sender_id == ss.ps.router.address{
                warn!("The destination {dst_id} is undeliverable")
            }
            else{
                // do routing
                if let Some(route) = ss.ps.router.routes.get(&sender_id){
                    if let Some(nh) = &route.next_hop{
                        if ss.os.log_routing {
                            info!("UND sender: {}, dst: {}, nh: {}", sender_id, dst_id, nh);
                        }
                        // forward packet
                        if let Some(link) = route.link{
                            if let Some(netlink) = ss.ps.links.get(&link){
                                let naddr = netlink.neigh_addr;
                                drop(ss);
                                send_packet(state.clone(), naddr, NetPacket::Undeliverable {
                                    dst_id,
                                    sender_id
                                }).await?;
                            }
                        }
                    }
                }
                // lets not send an undeliverable packet if i cant deliver it lmaooo
            }
        }
        NetPacket::TraceRoute { dst_id, sender_id, mut path } => {
            path.push(ss.ps.router.address.clone());
            if dst_id == ss.ps.router.address{
                drop(ss);
                send_routed_packet(state, RoutedPacket::TracedRoute {
                    path
                }, sender_id).await?;
            }
            else{
                // do routing
                if let Some(route) = ss.ps.router.routes.get(&dst_id){
                    if let Some(nh) = &route.next_hop{
                        if ss.os.log_routing {
                            info!("TRT sender: {}, dst: {}, nh: {}", sender_id, dst_id, nh);
                        }
                        // forward packet
                        if let Some(link) = route.link{
                            if let Some(netlink) = ss.ps.links.get(&link){
                                let naddr = netlink.neigh_addr;
                                drop(ss);
                                send_packet(state.clone(), naddr, NetPacket::TraceRoute {
                                    dst_id,
                                    sender_id,
                                    path
                                }).await?;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

async fn send_routed_packet(
    state: Arc<Mutex<SyncState>>,
    data: RoutedPacket,
    dst_id: <IPV4System as RoutingSystem>::NodeAddress,
) -> anyhow::Result<()> {
    let ss = state.lock().await;
    let cur_id = ss.ps.router.address.clone();
    drop(ss);
    // should probably handle next-hop undeliverable... but im lazy :)
    Box::pin(route_packet(state, data, dst_id, cur_id, None)).await?;
    Ok(())
}

async fn route_packet(
    state: Arc<Mutex<SyncState>>,
    data: RoutedPacket,
    dst_id: <IPV4System as RoutingSystem>::NodeAddress,
    sender_id: <IPV4System as RoutingSystem>::NodeAddress,
    prev_hop: Option<Ipv4Addr>
) -> anyhow::Result<()> {
    let mut ss = state.lock().await;
    if dst_id == ss.ps.router.address{
        drop(ss);
        handle_routed_packet(state, data, sender_id).await?;
    }
    else{
        // do routing
        if let Some(route) = ss.ps.router.routes.get(&dst_id){
            if let Some(nh) = &route.next_hop{
                if ss.os.log_routing {
                    info!("DP sender: {}, dst: {}, nh: {}", sender_id, dst_id, nh);
                }
                // forward packet
                if let Some(link) = route.link{
                    if let Some(netlink) = ss.ps.links.get(&link){
                        let naddr = netlink.neigh_addr;
                        drop(ss);
                        send_packet(state.clone(), naddr, NetPacket::Deliver {
                            dst_id,
                            sender_id, data
                        }).await?;
                        return Ok(())
                    }
                }
            }
        }
        // undeliverable
        if let Some(addr) = prev_hop{
            drop(ss);
            send_packet(state, addr, NetPacket::Undeliverable {
                dst_id,
                sender_id
            }).await?;
        }
    }
    Ok(())
}

async fn handle_routed_packet(
    state: Arc<Mutex<SyncState>>,
    pkt: RoutedPacket,
    src: <IPV4System as RoutingSystem>::NodeAddress
) -> anyhow::Result<()> {
    let mut ss = state.lock().await;
    
    match pkt {
        RoutedPacket::Ping => {
            drop(ss);
            send_routed_packet(state, RoutedPacket::Pong, src).await?;
        }
        RoutedPacket::Pong => {
            if let Some(start) = ss.os.pings.remove(&src){
                info!("Pong from {src} {:?}", (Instant::now() - start) / 2);
            }
        }
        RoutedPacket::TracedRoute { path } => {
            info!("Traced route from {src}: {}", path.join(" -> "));
        }
        RoutedPacket::Message(msg) => {
            info!("{src}> {msg}")
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    set_max_level(LevelFilter::Debug);
    set_boxed_logger(TermLogger::new(LevelFilter::Debug, Config::default(), TerminalMode::Mixed, ColorChoice::Auto)).expect("Failed to init logger");

    let (sender, recv) = tokio::sync::mpsc::channel(1000);

    info!("Starting Root Routing Demo");
    warn!("Notice: THIS DEMO IS NOT DESIGNED FOR SECURITY, AND SHOULD NEVER BE USED OUTSIDE OF A TEST ENVIRONMENT");

    let saved_state = if let Ok(file) = fs::read_to_string("./config.json").await {
        serde_json::from_str(&file)?
    } else {
        setup().await?
    };

    save_state(&saved_state).await?;
    let mut i_os = OperatingState::default();
    i_os.packet_queue = Some(sender);
    
    let ss = SyncState{
        ps: saved_state,
        os: i_os
    };
    let ssm = Arc::new(Mutex::new(ss));

    let handles = [
        tokio::spawn(server(ssm.clone())),
        tokio::spawn(ping_updater(ssm.clone())),
        tokio::spawn(route_updater(ssm.clone())),
        tokio::spawn(packet_sender(recv)),
    ];

    // handle I/O

    info!("Type \"help\" for help");

    let iter = stdin().lock().lines();
    for line in iter {
        let input = match line {
            Ok(x) => { Ok(x) }
            Err(err) => {
                let ss = ssm.lock().await;
                save_state(&ss.ps).await?;
                Err(err)
            }
        }?;
        let mut ss = ssm.lock().await;
        let split: Vec<&str> = input.split_whitespace().collect();
        if split.is_empty() {
            continue;
        }
        match split[0] {
            "help" => {
                info!(r#"Help:
                - help -- shows this page
                - exit -- exits and saves state
                [direct link]
                - ls -- lists all direct links
                - dping <link id> -- pings a direct neighbour
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
                rtable.push(format!("Self: {}, seq: {}", ss.ps.router.address, ss.ps.router.seqno));
                for (addr, route) in &ss.ps.router.routes {
                    rtable.push(
                        format!("{addr} - via: {}, nh: {}, c: {}, seq: {}, fd: {}, ret: {}",
                                route.link.unwrap_or(Uuid::nil()),
                                route.next_hop.clone().unwrap_or("?".to_string()),
                                route.metric,
                                route.source.data.seqno,
                                route.fd.unwrap_or(INF),
                                route.retracted
                        ))
                }
                info!("{}", rtable.join("\n"));
            }
            "rpkt" => {
                ss.os.log_routing = !ss.os.log_routing;
            }
            "dpkt" => {
                ss.os.log_delivery = !ss.os.log_delivery;
            }
            "exit" => {
                break;
            }
            "ping" => {
                if split.len() != 2 {
                    error!("Expected one argument");
                    continue;
                }
                let node = split[1];
                ss.os.pings.insert(node.to_string(), Instant::now());
                drop(ss);
                send_routed_packet(ssm.clone(), RoutedPacket::Ping, node.to_string()).await?;
            }
            "traceroute" | "tr" => {
                if split.len() != 2 {
                    error!("Expected one argument");
                    continue;
                }
                let node = split[1];
                if let Some(nh) = ss.ps.router.routes.get(node){
                    if let Some(link) = nh.link{
                        if let Some(netlink) = ss.ps.links.get(&link){
                            let s_addr = ss.ps.router.address.clone();
                            let naddr = netlink.neigh_addr;
                            drop(ss);
                            send_packet(ssm.clone(), naddr, TraceRoute {
                                path: vec![],
                                dst_id: node.to_string(),
                                sender_id: s_addr
                            }).await?;
                        }
                    }
                }
            }
            "msg" => {
                if split.len() <= 2 {
                    error!("Expected at least two arguments");
                    continue;
                }
                let node = split[1];
                let msg = split[2..].join(" ");
                drop(ss);
                send_routed_packet(ssm.clone(), RoutedPacket::Message(msg), node.to_string()).await?;
            }
            "ls" => {
                for (id, net) in &ss.ps.links {
                    if let Some(health) = ss.os.health.get(id) {
                        info!("id: {id}, addr: {}, ping: {:?}", net.neigh_addr, health.ping)
                    } else {
                        info!("id: {id}, addr: {} UNCONNECTED", net.neigh_addr)
                    }
                }
            }
            "dping" => {
                if split.len() != 2 {
                    error!("Expected one argument");
                    continue;
                }
                if let Some((id, link)) = ss.ps.links.iter().find(|(id, _)| id.to_string() == split[1]) {
                    let naddr = link.neigh_addr;
                    let nid = *id;
                    drop(ss);
                    ping(ssm.clone(), nid, naddr, false).await;
                } else {
                    warn!("No ")
                }
            }
            "link" => {
                if split.len() != 2 {
                    error!("Expected one argument");
                    continue;
                }
                let ip = Ipv4Addr::from_str(split[1]);
                match ip {
                    Err(parse) => {
                        error!("Failed to parse ip, {parse}")
                    }
                    Ok(ip) => {
                        let id = Uuid::new_v4();
                        info!("Sent linking request {id} to {ip}");
                        ss.os.unlinked.insert(id, NetLink {
                            link: id,
                            neigh_addr: ip,
                            neigh_node: "UNKNOWN".to_string(),
                        });
                        let from_addr = ss.ps.router.address.clone();
                        drop(ss);
                        send_packet(ssm.clone(), ip, LinkRequest {
                            from: from_addr,
                            link_id: id,
                        }).await?;
                    }
                }
            }
            "alink" => {
                if split.len() != 2 {
                    error!("Expected one argument");
                    continue;
                }
                if let Some(netlink) = ss.os.link_requests.remove(split[1]) {
                    ss.ps.router.links.insert(
                        netlink.link,
                        Neighbour {
                            addr: netlink.neigh_node.clone(),
                            link: netlink.link,
                            link_cost: INF,
                            routes: HashMap::new(),
                        },
                    );
                    let node_addr = ss.ps.router.address.clone();
                    let lid = netlink.link;
                    let naddr = netlink.neigh_addr;
                    ss.ps.links.insert(netlink.link, netlink);
                    drop(ss);
                    send_packet(ssm.clone(), naddr, NetPacket::LinkResponse {
                        link_id: lid,
                        node_id: node_addr,
                    }).await?;
                    info!("LINKING SUCCESS");
                } else {
                    error!("No matching linking code found!");
                }
            }
            "dlink" => {
                if split.len() != 2 {
                    error!("Expected one argument");
                    continue;
                }
                let id = Uuid::parse_str(split[1]);
                match id {
                    Ok(uuid) => {
                        ss.ps.links.remove(&uuid);
                        ss.os.health.remove(&uuid);
                    }
                    Err(_) => {
                        error!("Invalid UUID")
                    }
                }
            }
            &_ => {
                error!("Unknown command, please try again or type \"help\" for help.")
            }
        }
    }

    let ss = ssm.lock().await;
    save_state(&ss.ps).await?;

    Ok(())
}