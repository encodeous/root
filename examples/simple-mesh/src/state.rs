use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use root::router::Router;
use uuid::Uuid;
use root::framework::RoutingSystem;
use crate::link::NetLink;
use crate::routing::IPV4System;
use serde_with::serde_as;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio_serde::formats::Json;
use tokio_util::codec::{Framed, FramedRead, FramedWrite, LengthDelimitedCodec};
use crate::packet::NetPacket;

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
    pub ping_start: Instant,
}

#[derive(Default)]
pub struct OperatingState {
    pub health: HashMap<<IPV4System as RoutingSystem>::Link, LinkHealth>,
    pub unlinked: HashMap<<IPV4System as RoutingSystem>::Link, NetLink>,
    pub link_requests: HashMap<<IPV4System as RoutingSystem>::NodeAddress, NetLink>,
    pub pings: HashMap<<IPV4System as RoutingSystem>::NodeAddress, Instant>,
    pub packet_queue: Option<Sender<(Ipv4Addr, NetPacket)>>,
    pub log_routing: bool,
    pub log_delivery: bool,
}

pub struct SyncState{
    pub ps: PersistentState,
    pub os: OperatingState
}