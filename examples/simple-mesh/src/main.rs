mod link;
mod routing;
mod state;
mod packet;
mod mesh_router;

use std::cmp::max;
use std::collections::{HashMap, HashSet};
use std::env;
use std::io::{BufRead, stdin};
use std::net::{IpAddr, Ipv4Addr, SocketAddrV4};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use futures::SinkExt;
use futures::TryStreamExt;
use inquire::{prompt_text};
use log::{debug, error, info, set_boxed_logger, set_max_level, warn};
use serde_json::json;
use tokio::fs;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{Receiver};
use tokio::time::{sleep, timeout};
use root::router::{DummyMAC, INF, Router};
use crate::state::{LinkHealth, OperatingState, PersistentState, SyncState};
use crate::routing::IPV4System;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use uuid::{Uuid};
use root::add;
use root::concepts::neighbour::Neighbour;
use root::concepts::packet::OutboundPacket;
use root::framework::RoutingSystem;
use crate::link::NetLink;
use crate::mesh_router::start_router;
use crate::packet::{NetPacket, RoutedPacket};
use crate::packet::NetPacket::{LinkRequest, Ping, Pong, TraceRoute};
use crate::state::MainLoopEvent::DispatchCommand;

macro_rules! ml {
  ( $mutex_arc:expr ) => {
    $mutex_arc.lock().unwrap()
  };
}

async fn save_state(state: Arc<Mutex<SyncState>>) -> anyhow::Result<()> {
    let content = {
        serde_json::to_vec(&ml!(state).ps)?
    };
    fs::write("./config.json", content).await?;
    debug!("Saved State");
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


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info")
    }
    env_logger::init();

    info!("Starting Root Routing Demo");
    warn!("Notice: THIS DEMO IS NOT DESIGNED FOR SECURITY, AND SHOULD NEVER BE USED OUTSIDE OF A TEST ENVIRONMENT");

    info!("Type \"help\" for help");
    
    let mut saved_state = if let Ok(file) = fs::read_to_string("./config.json").await {
        serde_json::from_str(&file)?
    } else {
        setup().await?
    };

    for (link, netlink) in &saved_state.links{
        saved_state.router.links.insert(
            *link,
            Neighbour {
                addr: netlink.neigh_node.clone(),
                link: netlink.link,
                link_cost: INF,
                routes: HashMap::new(),
            },
        );
    }
    
    let mq = start_router(saved_state, OperatingState{
        health: Default::default(),
        unlinked: Default::default(),
        link_requests: Default::default(),
        pings: Default::default(),

        log_routing: false,
        log_delivery: false,
    });
    
    let mut input_buf = String::new();
    
    while !mq.cancellation_token.is_cancelled(){
        stdin().read_line(&mut input_buf)?;
        mq.main.send(DispatchCommand(input_buf))?;
        input_buf = String::new();
    }
    
    mq.cancellation_token.cancel();
    
    sleep(Duration::from_secs(1)).await; // wait for main thread to finish

    Ok(())
}