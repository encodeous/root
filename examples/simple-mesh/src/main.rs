mod link;
mod routing;
mod state;
mod packet;

use std::collections::HashMap;
use std::fmt::format;
use std::io::{BufRead, stdin};
use std::io::ErrorKind::ConnectionRefused;
use std::net::{IpAddr, Ipv4Addr, SocketAddrV4};
use std::net::SocketAddr::V4;
use std::ops::DerefMut;
use std::str::FromStr;
use std::sync::{Arc};
use std::time::{Duration, Instant};
use anyhow::{anyhow, Context};
use futures::{AsyncWriteExt, SinkExt, TryStreamExt};
use futures::future::err;
use inquire::{InquireError, MultiSelect, prompt_text, prompt_u32};
use inquire::error::InquireResult;
use inquire::InquireError::OperationCanceled;
use inquire::list_option::ListOption;
use inquire::validator::Validation;
use log::{debug, error, info, set_boxed_logger, set_max_level, warn};
use netdev::ip::Ipv4Net;
use serde::de::Unexpected::Str;
use serde_json::json;
use simplelog::*;
use tokio::fs;
use tokio::io::{AsyncReadExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::time::sleep;
use root::router::{DummyMAC, INF, Router};
use crate::state::{LinkHealth, OperatingState, PersistentState};
use crate::routing::IPV4System;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use tokio_serde::{Serializer, Deserializer, Framed, SymmetricallyFramed};
use tokio_serde::formats::SymmetricalJson;
use uuid::{Error, Uuid};
use root::concepts::neighbour::Neighbour;
use root::framework::LinkAddress;
use crate::link::NetLink;
use crate::packet::NetPacket;
use crate::packet::NetPacket::{LinkRequest, Ping, Pong};

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

async fn server(state: Arc<Mutex<PersistentState>>, op_state: Arc<Mutex<OperatingState>>) -> anyhow::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:9988").await?;

    loop {
        let (sock, addr) = listener.accept().await?;

        debug!("Got packet from {addr}");

        let len_del = FramedRead::new(sock, LengthDelimitedCodec::new());
        let mut deserialized = SymmetricallyFramed::new(len_del, SymmetricalJson::<NetPacket>::default());
        if let IpAddr::V4(addr) = addr.ip() {
            tokio::spawn({
                let c_state = state.clone();
                let c_op_state = op_state.clone();
                let c_addr = addr.clone();
                async move {
                    while let Some(msg) = deserialized.try_next().await.unwrap() {
                        match handle_packet(c_state.clone(), c_op_state.clone(), msg, &c_addr).await {
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

async fn ping(op_state: Arc<Mutex<OperatingState>>, id: Uuid, addr: Ipv4Addr, silent: bool) {
    {
        let mut os = op_state.lock().await;
        let entry = os.health.entry(id).or_insert(
            LinkHealth {
                ping: Duration::MAX,
                ping_start: Instant::now(),
                last_ping: Instant::now(),
            }
        );
        entry.ping_start = Instant::now();
    }
    tokio::spawn(async move {
        if let Err(_) = send_packets_wait(addr, vec![Ping(id, silent)]).await {
            let mut os = op_state.lock().await;
            os.health.entry(id).and_modify(|x| {
                x.ping = Duration::MAX
            });
        }
    });
}

async fn ping_updater(state: Arc<Mutex<PersistentState>>, op_state: Arc<Mutex<OperatingState>>) -> anyhow::Result<()> {
    loop {
        sleep(Duration::from_millis(5000)).await;
        let mut cs = state.lock().await;
        for (lid, nlink) in &cs.links {
            ping(op_state.clone(), *lid, nlink.neigh_addr, true).await;
        }
    }
}
async fn route_updater(state: Arc<Mutex<PersistentState>>) -> anyhow::Result<()> {
    loop {
        sleep(Duration::from_millis(5000)).await;
        let mut cs = state.lock().await;
        cs.router.full_update();
        send_outbound(cs.deref_mut());

        save_state(&cs).await?;
    }
}

fn send_outbound(cs: &mut PersistentState) {
    let mut dsts = HashMap::new();
    for pkt in cs.router.outbound_packets.drain(..) {
        let entry = dsts.entry(pkt.link_addr.clone()).or_insert_with(|| {
            vec![]
        });
        entry.push(NetPacket::Routing {
            link_id: pkt.link_addr.0,
            data: pkt.packet.data,
        });
    }

    for (dst, data) in dsts {
        if let Some(netaddr) = cs.links.get(&dst.0) {
            send_packets(netaddr.neigh_addr, data);
        }
    }
}

fn send_packets(addr: Ipv4Addr, pkts: Vec<NetPacket>) {
    tokio::spawn(async move {
        let res = TcpStream::connect(SocketAddrV4::new(addr, 9988)).await;
        match res {
            Ok(stream) => {
                let len_del = FramedWrite::new(stream, LengthDelimitedCodec::new());
                let mut serialized = SymmetricallyFramed::new(len_del, SymmetricalJson::<NetPacket>::default());
                for pkt in pkts {
                    serialized.send(pkt).await.unwrap();
                }
            }
            Err(err) => {
                if (err.kind() != ConnectionRefused) {
                    anyhow::Result::<()>::Err(anyhow!(err)).unwrap();
                }
            }
        };
    });
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

fn send_packet(addr: Ipv4Addr, pkt: NetPacket) {
    send_packets(addr, vec![pkt]);
}

fn update_link_health(cs: &mut PersistentState, link: Uuid, link_health: &LinkHealth){
    if let Some(netlink) = cs.links.get(&link){
        let link_addr = (link, netlink.neigh_node.clone());
        if let Some(neigh) = cs.router.links.get_mut(&link_addr){
            neigh.link_cost = {
                if link_health.ping == Duration::MAX{
                    INF
                }
                else{
                    link_health.ping.as_millis() as u16
                }
            }
        }
        cs.router.update();
    }
}

async fn handle_packet(state: Arc<Mutex<PersistentState>>, op_state: Arc<Mutex<OperatingState>>, pkt: NetPacket, addr: &Ipv4Addr) -> anyhow::Result<()> {
    let mut cs = state.lock().await;
    let mut os = op_state.lock().await;

    match pkt {
        Ping(id, silent) => {
            if let Some(link) = cs.links.get(&id) {
                debug!("Ping received from {} nid: {}", link.neigh_addr, link.neigh_node);
                send_packets(link.neigh_addr, vec![Pong(id, silent)]);
            }
        }
        Pong(id, silent) => {
            if let Some(link) = cs.links.get(&id) {
                if silent {
                    debug!("Pong received from {}", link.neigh_addr);
                } else {
                    info!("Pong received from {}", link.neigh_addr);
                }
                // update link timing
                if let Some(health) = os.health.get_mut(&id) {
                    health.last_ping = Instant::now();
                    health.ping = (Instant::now() - health.ping_start) / 2;
                    update_link_health(cs.deref_mut(), id, health);
                }
            }
        }
        NetPacket::Routing { link_id, data } => {
            if let Some(link) = cs.links.get(&link_id) {
                let link_addr = (link.link, link.neigh_node.clone());
                info!("Routing Packet: {}", json!(data));
                cs.router.handle_packet(&DummyMAC::from(data), &link_addr);
                cs.router.update();
                send_outbound(cs.deref_mut());
            }
        }
        LinkRequest { link_id, from } => {
            info!("LINKING REQUEST: {link_id} from {from}. Type \"alink {link_id}\" to accept.");
            os.link_requests.insert(link_id, NetLink {
                link: link_id,
                neigh_addr: addr.clone(),
                neigh_node: from,
            });
        }
        NetPacket::LinkResponse { link_id, node_id } => {
            info!("LINKING SUCCESS: {node_id} has accepted the link {link_id}.");
            if let Some(mut net_link) = os.unlinked.remove(&link_id) {
                net_link.neigh_node = node_id.clone();
                let link = (link_id, node_id.clone());
                cs.router.links.insert(
                    link.clone(),
                    Neighbour {
                        addr: node_id.clone(),
                        link: link_id,
                        link_cost: INF,
                        routes: HashMap::new(),
                    },
                );
                cs.links.insert(link_id, net_link);
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    set_max_level(LevelFilter::Info);
    set_boxed_logger(TermLogger::new(LevelFilter::Info, Config::default(), TerminalMode::Mixed, ColorChoice::Auto)).expect("Failed to init logger");

    info!("Starting Root Routing Demo");
    warn!("Notice: THIS DEMO IS NOT DESIGNED FOR SECURITY, AND SHOULD NEVER BE USED OUTSIDE OF A TEST ENVIRONMENT");

    let saved_state = if let Ok(file) = fs::read_to_string("./config.json").await {
        serde_json::from_str(&file)?
    } else {
        setup().await?
    };

    save_state(&saved_state).await?;

    let per_state = Arc::new(Mutex::new(saved_state));
    let op_state = Arc::new(Mutex::new(OperatingState::default()));

    let handles = [
        tokio::spawn(server(per_state.clone(), op_state.clone())),
        tokio::spawn(ping_updater(per_state.clone(), op_state.clone())),
        tokio::spawn(route_updater(per_state.clone()))
    ];

    // handle I/O

    info!("Type \"help\" for help");

    let iter = stdin().lock().lines();
    for line in iter {
        let input = match line {
            Ok(x) => { Ok(x) }
            Err(err) => {
                let cs = per_state.lock().await;
                save_state(&cs).await?;
                Err(err)
            }
        }?;
        let mut cs = per_state.lock().await;
        let mut os = op_state.lock().await;
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
                - nh -- gets next hop to node
                - ping -- pings node
                - msg -- sends a message to a node
                - tracert -- traces a route to a node
                "#);
            }
            "route" => {
                let mut rtable = vec![];
                info!("Route Table:");
                for (addr, route) in &cs.router.routes {
                    rtable.push(
                        format!("{addr} - nh: {}, c: {}, via: {}, seq: {}",
                                route.next_hop.clone().unwrap_or("?".to_string()),
                                route.metric,
                                route.link.unwrap_or(Uuid::nil()),
                                route.source.data.seqno
                        ))
                }
                info!("{}", rtable.join("\n"));
            }
            "exit" => {
                break;
            }
            "ls" => {
                for (id, net) in &cs.links {
                    if let Some(health) = os.health.get(id) {
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
                if let Some((id, link)) = cs.links.iter().find(|(id, _)| id.to_string() == split[1]) {
                    drop(os); // need to drop, otherwise we have a deadlock
                    ping(op_state.clone(), *id, link.neigh_addr, false).await;
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
                        os.unlinked.insert(id, NetLink {
                            link: id,
                            neigh_addr: ip,
                            neigh_node: "UNKNOWN".to_string(),
                        });
                        send_packet(ip, LinkRequest {
                            from: cs.router.address.clone(),
                            link_id: id,
                        })
                    }
                }
            }
            "alink" => {
                if split.len() != 2 {
                    error!("Expected one argument");
                    continue;
                }
                let id = Uuid::parse_str(split[1]);
                match id {
                    Ok(uuid) => {
                        if let Some(netlink) = os.link_requests.remove(&uuid) {
                            let link_addr = (netlink.link, netlink.neigh_node.clone());
                            cs.router.links.insert(
                                link_addr.clone(),
                                Neighbour {
                                    addr: netlink.neigh_node.clone(),
                                    link: netlink.link,
                                    link_cost: INF,
                                    routes: HashMap::new(),
                                },
                            );
                            send_packet(netlink.neigh_addr, NetPacket::LinkResponse {
                                link_id: netlink.link,
                                node_id: cs.router.address.clone(),
                            });
                            cs.links.insert(netlink.link, netlink);
                            info!("LINKING SUCCESS");
                        } else {
                            error!("No matching linking code found!");
                        }
                    }
                    Err(_) => {
                        error!("Invalid UUID")
                    }
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
                        cs.links.remove(&uuid);
                        os.health.remove(&uuid);
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

    let cs = per_state.lock().await;
    save_state(&cs).await?;

    Ok(())
}