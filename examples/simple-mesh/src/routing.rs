use std::fmt::Display;
use std::net::Ipv4Addr;

use serde::{Deserialize, Serialize};

use root::framework::RoutingSystem;
use root::router::{NoMACSystem, Router};

pub struct IPV4System {}

#[derive(Eq, PartialEq, Hash, Serialize, Deserialize, Clone)]
pub enum NType {
    PhysicalIP,
}
impl RoutingSystem for IPV4System {
    type NodeAddress = u32;
    type PhysicalAddress = Ipv4Addr;
    type NetworkType = NType;
    type InterfaceId = u32;
    type MACSystem = NoMACSystem;
    type DedupType = [u8; 16];
}
