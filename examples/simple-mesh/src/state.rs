use std::collections::HashMap;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use root::router::Router;
use uuid::Uuid;
use root::framework::RoutingSystem;
use crate::link::NetLink;
use crate::routing::IPV4System;
use serde_with::serde_as;

#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct PersistentState {
    #[serde_as(as = "Vec<(_, _)>")]
    pub links: HashMap<<IPV4System as RoutingSystem>::Link, NetLink>,
    pub router: Router<IPV4System>
}

pub struct LinkHealth{
    pub last_ping: Instant,
    pub ping: Duration,
    pub ping_start: Instant
}

#[derive(Default)]
pub struct OperatingState {
    pub health: HashMap<<IPV4System as RoutingSystem>::Link, LinkHealth>,
    pub unlinked: HashMap<<IPV4System as RoutingSystem>::Link, NetLink>,
    pub link_requests: HashMap<<IPV4System as RoutingSystem>::Link, NetLink>
}