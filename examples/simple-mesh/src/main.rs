mod link;
mod routing;
mod state;
mod packet;

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddrV4};
use std::net::SocketAddr::V4;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use anyhow::Context;
use futures::{AsyncWriteExt, SinkExt, TryStreamExt};
use inquire::{MultiSelect, prompt_u32};
use inquire::list_option::ListOption;
use inquire::validator::Validation;
use log::{debug, error, info, warn};
use netdev::ip::Ipv4Net;
use serde_json::json;
use simplelog::*;
use tokio::fs;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use root::router::Router;
use crate::state::{OperatingState, PersistentState};
use crate::routing::IPV4System;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use tokio_serde::{Serializer, Deserializer, Framed, SymmetricallyFramed};
use tokio_serde::formats::SymmetricalJson;
use crate::packet::NetPacket;
use crate::packet::NetPacket::Pong;

async fn save_state(cfg: &PersistentState) -> anyhow::Result<()> {
    fs::write("./config.json", serde_json::to_vec(cfg)?).await?;
    Ok(())
}

async fn setup() -> anyhow::Result<PersistentState> {
    info!("Node Setup (First Time):");
    let id = prompt_u32("Pick a unique node id (u32): ")?;

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

        tokio::spawn({
            let c_state = state.clone();
            let c_op_state = op_state.clone();
            async move {
                while let Some(msg) = deserialized.try_next().await.unwrap() {
                    match handle_packet(c_state.clone(), c_op_state.clone(), msg).await {
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

fn send_packets(addr: Ipv4Addr, pkts: Vec<NetPacket>){
    tokio::spawn(async move {
        let stream = TcpStream::connect(SocketAddrV4::new(addr, 9988)).await.unwrap();
        let len_del = FramedWrite::new(stream, LengthDelimitedCodec::new());
        let mut serialized = SymmetricallyFramed::new(len_del, SymmetricalJson::<NetPacket>::default());
        for pkt in pkts{
            serialized.send(pkt).await.unwrap();
        }
    });
}

async fn handle_packet(state: Arc<Mutex<PersistentState>>, op_state: Arc<Mutex<OperatingState>>, pkt: NetPacket) -> anyhow::Result<()> {
    let mut cs = state.lock().unwrap();
    let mut os = op_state.lock().unwrap();

    match pkt {
        NetPacket::Ping(id) => {
            if let Some(link) = cs.links.get(&id){
                debug!("Ping received from {}", link.neigh_addr);
                send_packets(link.neigh_addr, vec![Pong(id)]);
            }
        }
        NetPacket::Pong(id) => {
            if let Some(link) = cs.links.get(&id){
                debug!("Pong received from {}", link.neigh_addr);
                
            }
        }
        NetPacket::Routing { link_id, data } => {
            
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    CombinedLogger::init(
        vec![
            TermLogger::new(LevelFilter::Debug, Config::default(), TerminalMode::Mixed, ColorChoice::Auto)
        ]
    ).unwrap();

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

    let handles = vec![
        tokio::spawn(server(per_state.clone(), op_state.clone()))
    ];

    futures::future::join_all(handles).await;

    Ok(())
}
