use serde::{Deserialize, Serialize};

use root::framework::RoutingSystem;
use root::router::NoMACSystem;

pub struct IPV4System {}

#[derive(Eq, PartialEq, Hash, Serialize, Deserialize, Clone)]
pub enum NType {
    PhysicalIP,
}
impl RoutingSystem for IPV4System {
    type NodeAddress = String;
    type Link = u32;
    type MACSystem = NoMACSystem;
}
