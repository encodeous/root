mod link;
mod routing;
mod state;
mod packet;

use std::collections::HashMap;
use std::io::{BufRead, stdin};
use std::io::ErrorKind::ConnectionRefused;
use std::net::{IpAddr, Ipv4Addr, SocketAddrV4};
use std::net::SocketAddr::V4;
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

async fn ping_updater(state: Arc<Mutex<PersistentState>>, op_state: Arc<Mutex<OperatingState>>) -> anyhow::Result<()> {
    loop {
        sleep(Duration::from_millis(5000)).await;
        let mut cs = state.lock().await;
        let mut os = op_state.lock().await;
        for (lid, nlink) in &cs.links{
            os.health.entry(*lid).or_insert(
                LinkHealth {
                    ping: Duration::from_millis(100),
                    ping_start: Instant::now(),
                    last_ping: Instant::now()
                }
            );

            send_packets(nlink.neigh_addr, vec![Ping(*lid, true)]);
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
                if(err.kind() != ConnectionRefused) {
                    anyhow::Result::<()>::Err(anyhow!(err)).unwrap();
                }
            }
        };
    });
}

fn send_packet(addr: Ipv4Addr, pkt: NetPacket) {
    send_packets(addr, vec![pkt]);
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
                }
            }
        }
        NetPacket::Routing { link_id, data } => {
            if let Some(link) = cs.links.get(&link_id) {
                let link_addr = (link.link.clone(), link.neigh_node.clone());
                cs.router.handle_packet(&DummyMAC::from(data), &link_addr);
            }
        }
        LinkRequest { link_id, from } => {
            info!("LINKING REQUEST: {link_id} from {from}. Type \"alink {link_id}\" to accept.");
            os.link_requests.insert(link_id, NetLink {
                link: link_id,
                neigh_addr: addr.clone(),
                neigh_node: from
            });
        }
        NetPacket::LinkResponse { link_id, node_id } => {
            info!("LINKING SUCCESS: {node_id} has accepted the link {link_id}.");
            if let Some(mut net_link) = os.unlinked.remove(&link_id) {
                net_link.neigh_node = node_id.clone();
                let link = (link_id, node_id.clone());
                cs.router.links.insert(
                    link.clone(),
                    Neighbour{
                        addr: node_id.clone(),
                        link: link_id,
                        link_cost: INF,
                        routes: HashMap::new()
                    }
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
        tokio::spawn(ping_updater(per_state.clone(), op_state.clone()))
    ];

    // handle I/O

    info!("Type \"help\" for help");

    let iter = stdin().lock().lines();
    for line in iter{
        let input = match line {
            Ok(x) => {Ok(x)}
            Err(err) => {
                let cs = per_state.lock().await;
                save_state(&cs).await?;
                Err(err)
            }
        }?;
        let mut cs = per_state.lock().await;
        let mut os = op_state.lock().await;
        let split: Vec<&str> = input.split_whitespace().collect();
        if split.is_empty(){
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
            "exit" => {
                break;
            }
            "ls" => {
                for (id, net) in &cs.links {
                    if let Some(health) = os.health.get(id){
                        info!("id: {id}, addr: {}, ping: {:?}", net.neigh_addr, health.ping)
                    }
                    else{
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
                    send_packet(link.neigh_addr, Ping(*id, false));
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
                        if let Some(netlink) = os.link_requests.remove(&uuid){
                            let link_addr = (netlink.link, netlink.neigh_node.clone());
                            cs.router.links.insert(
                                link_addr.clone(),
                                Neighbour{
                                    addr: netlink.neigh_node.clone(),
                                    link: netlink.link,
                                    link_cost: INF,
                                    routes: HashMap::new()
                                }
                            );
                            send_packet(netlink.neigh_addr, NetPacket::LinkResponse {
                                link_id: netlink.link,
                                node_id: cs.router.address.clone()
                            });
                            cs.links.insert(netlink.link, netlink);
                            info!("LINKING SUCCESS");
                        }
                        else{
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