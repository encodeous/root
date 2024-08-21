use std::net::Ipv4Addr;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct NetLink {
    pub link_id: u32,
    pub neigh_id: u32,
    pub neigh_addr: Ipv4Addr
}