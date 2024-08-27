mod link;
mod routing;
mod state;
mod packet;
mod mesh_router;

use std::collections::{HashMap};
use std::{env, fs};
use std::io::{stdin};
use std::time::{Duration};
use inquire::{prompt_text};
use log::error;
use log::info;
use log::warn;
use tokio::time::sleep;
use root::router::{INF, Router};
use crate::state::OperatingState;
use crate::state::PersistentState;
use root::concepts::neighbour::Neighbour;
use crate::mesh_router::start_router;
use crate::state::MainLoopEvent::DispatchCommand;

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
    
    let mut saved_state = if let Ok(file) = fs::read_to_string("./config.json") {
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
    saved_state.router.links.retain(|k, _| {
        saved_state.links.contains_key(k)
    });
    
    let mq = start_router(saved_state, OperatingState{
        health: Default::default(),
        unlinked: Default::default(),
        link_requests: Default::default(),
        pings: Default::default(),

        log_routing: false,
        log_delivery: false,
    });
    
    let mut input_buf = String::new();

    let tmq = mq.clone();
    ctrlc::set_handler(move || {
        tmq.cancellation_token.cancel();
        tmq.main.send(DispatchCommand(String::new())).unwrap();
    }).expect("Error setting Ctrl-C handler");

    while !mq.cancellation_token.is_cancelled(){
        stdin().read_line(&mut input_buf)?;
        mq.main.send(DispatchCommand(input_buf))?;
        input_buf = String::new();
    }
    
    sleep(Duration::from_secs(1)).await; // wait for main thread to finish

    
    
    Ok(())
}