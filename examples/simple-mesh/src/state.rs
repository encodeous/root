use serde::{Deserialize, Serialize};
use root::router::Router;
use crate::routing::IPV4System;

#[derive(Serialize, Deserialize)]
pub struct MeshConfig {
    pub address: u32,
    pub seqno: u16,
    pub itf: Vec<u32>
}

#[derive(Serialize, Deserialize)]
pub struct RouterState{
    pub router: Router<IPV4System>
}