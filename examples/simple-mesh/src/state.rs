use std::collections::HashMap;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use root::router::Router;
use crate::link::NetLink;
use crate::routing::IPV4System;

#[derive(Serialize, Deserialize)]
pub struct PersistentState {
    pub links: HashMap<u32, NetLink>,
    pub router: Router<IPV4System>
}

pub struct LinkHealth{
    pub last_ping: Instant,
    pub ping: Duration,
}

#[derive(Default)]
pub struct OperatingState {
    pub netlink_pings: HashMap<u32, Instant>,
    pub health: HashMap<u32, LinkHealth>
}