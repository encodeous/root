use std::net::Ipv4Addr;
use serde::{Deserialize, Serialize};
use root::framework::{RoutingSystem};
use crate::routing::IPV4System;

#[derive(Serialize, Deserialize, Clone)]
pub struct NetLink {
    pub link: <IPV4System as RoutingSystem>::Link,
    pub neigh_node: <IPV4System as RoutingSystem>::NodeAddress,
    pub neigh_addr: Ipv4Addr
}