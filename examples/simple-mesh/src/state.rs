use std::collections::HashMap;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use root::router::Router;
use uuid::Uuid;
use crate::link::NetLink;
use crate::routing::IPV4System;

#[derive(Serialize, Deserialize)]
pub struct PersistentState {
    pub links: HashMap<Uuid, NetLink>,
    pub router: Router<IPV4System>
}

pub struct LinkHealth{
    pub last_ping: Instant,
    pub ping: Duration,
    pub ping_start: Instant
}

#[derive(Default)]
pub struct OperatingState {
    pub health: HashMap<Uuid, LinkHealth>
}