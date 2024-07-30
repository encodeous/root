mod neighbour;
mod routing;
mod state;

use std::str::FromStr;
use std::sync::{Arc, Mutex};
use inquire::{MultiSelect, prompt_u32};
use inquire::list_option::ListOption;
use inquire::validator::Validation;
use log::{info, warn};
use simplelog::*;
use tokio::fs;
use root::router::Router;
use crate::state::MeshConfig;
use crate::routing::IPV4System;

async fn save_config(cfg: &MeshConfig) -> anyhow::Result<()>{
    fs::write("./config.json", serde_json::to_vec(cfg)?).await?;
    Ok(())
}

async fn setup() -> anyhow::Result<MeshConfig>{
    info!("Node Setup (First Time):");
    let id = prompt_u32("Pick a unique node id (u32): ")?;

    info!("Set node id to {id}");

    let interfaces = netdev::get_interfaces();

    let dsp_interface = interfaces.iter()
        .filter(|x|
            !x.ipv4.is_empty()
        )
        .map(|itf| format!("{}: {} {:?} ({:?})", itf.index, itf.name, itf.friendly_name.clone().unwrap_or("".to_string()), itf.ipv4
        ))
        .collect();

    let validator = |a: &[ListOption<&String>]| {
        if a.is_empty() {
            return Ok(Validation::Invalid("Please pick at least one interface!".into()));
        }
        Ok(Validation::Valid)
    };

    let ans = MultiSelect::new("Select interfaces to bind/listen on:", dsp_interface)
        .with_validator(validator)
        .prompt()?;
    
    let sel_itf = ans.iter().map(|x| u32::from_str(x.split_once(":").unwrap().0).unwrap()).collect();

    Ok(MeshConfig {
        address: id,
        seqno: 0,
        itf: sel_itf
    })
}

async fn server(cfg: Arc<Mutex<MeshConfig>>, system: Arc<Mutex<IPV4System>>){

}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    CombinedLogger::init(
        vec![
            TermLogger::new(LevelFilter::Warn, Config::default(), TerminalMode::Mixed, ColorChoice::Auto)
        ]
    ).unwrap();

    info!("Starting Root Routing Demo");
    warn!("Notice: THIS DEMO IS HIGHLY INSECURE, AND SHOULD NEVER BE USED OUTSIDE OF A TEST ENVIRONMENT");

    let config = if let Ok(file) = fs::read_to_string("./config.json").await{
        serde_json::from_str(&file)?
    }
    else{
        setup().await?
    };

    save_config(&config).await?;
    
    

    let system = Arc::new(Mutex::new(IPV4System{}));

    let shared_cfg = Arc::new(Mutex::new(config));

    tokio::spawn(server(shared_cfg, system));


    let interfaces = netdev::get_interfaces();
    for interface in interfaces {
        println!("Interface:");
        println!("\tIndex: {}", interface.index);
        println!("\tName: {}", interface.name);
        println!("\tFriendly Name: {:?}", interface.friendly_name);
        println!("\tDescription: {:?}", interface.description);
        println!("\tType: {}", interface.if_type.name());
        println!("\tFlags: {:?}", interface.flags);
        println!("\t\tis UP {}", interface.is_up());
        println!("\t\tis LOOPBACK {}", interface.is_loopback());
        println!("\t\tis MULTICAST {}", interface.is_multicast());
        println!("\t\tis BROADCAST {}", interface.is_broadcast());
        println!("\t\tis POINT TO POINT {}", interface.is_point_to_point());
        println!("\t\tis TUN {}", interface.is_tun());
        println!("\t\tis RUNNING {}", interface.is_running());
        println!("\t\tis PHYSICAL {}", interface.is_physical());
        if let Some(mac_addr) = interface.mac_addr {
            println!("\tMAC Address: {}", mac_addr);
        } else {
            println!("\tMAC Address: (Failed to get mac address)");
        }
        println!("\tIPv4: {:?}", interface.ipv4);
        println!("\tIPv6: {:?}", interface.ipv6);
        println!("\tTransmit Speed: {:?}", interface.transmit_speed);
        println!("\tReceive Speed: {:?}", interface.receive_speed);
        if let Some(gateway) = interface.gateway {
            println!("Gateway");
            println!("\tMAC Address: {}", gateway.mac_addr);
            println!("\tIPv4 Address: {:?}", gateway.ipv4);
            println!("\tIPv6 Address: {:?}", gateway.ipv6);
        } else {
            println!("Gateway: (Not found)");
        }
        println!("DNS Servers: {:?}", interface.dns_servers);
        println!("Default: {}", interface.default);
        println!();
    }
    Ok(())
}
