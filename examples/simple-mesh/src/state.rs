use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::time::{Duration, Instant};
use crossbeam_channel::Sender;
use serde::{Deserialize, Serialize};
use root::router::Router;
use uuid::Uuid;
use root::framework::RoutingSystem;
use crate::link::NetLink;
use crate::routing::IPV4System;
use serde_with::serde_as;
use tokio_util::codec::{Framed, FramedRead, FramedWrite, LengthDelimitedCodec};
use tokio_util::sync::CancellationToken;
use crate::packet::{NetPacket, RoutedPacket};

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

pub struct OperatingState {
    pub health: HashMap<<IPV4System as RoutingSystem>::Link, LinkHealth>,
    pub unlinked: HashMap<<IPV4System as RoutingSystem>::Link, NetLink>,
    pub link_requests: HashMap<<IPV4System as RoutingSystem>::NodeAddress, NetLink>,
    pub pings: HashMap<<IPV4System as RoutingSystem>::NodeAddress, Instant>,
    pub log_routing: bool,
    pub log_delivery: bool,
}

pub struct SyncState{
    pub ps: PersistentState,
    pub os: OperatingState
}

#[derive(Clone)]
pub struct MessageQueue{
    pub main: Sender<MainLoopEvent>,
    pub outbound: Sender<QueuedPacket>,
    pub cancellation_token: CancellationToken
}

pub struct QueuedPacket {
    pub to: Ipv4Addr,
    pub packet: NetPacket,
    pub failure_event: MainLoopEvent
}

#[derive(Serialize, Deserialize)]
pub enum MainLoopEvent{
    InboundPacket{
        address: Ipv4Addr,
        packet: NetPacket
    },
    RoutePacket{
        to: <IPV4System as RoutingSystem>::NodeAddress,
        from: <IPV4System as RoutingSystem>::NodeAddress,
        packet: RoutedPacket
    },
    DispatchPingLink{
        link_id: <IPV4System as RoutingSystem>::Link
    },
    PingResultFailed{
        link_id: <IPV4System as RoutingSystem>::Link
    },
    DispatchCommand(String),
    TimerRouteUpdate,
    TimerPingUpdate,
    Shutdown,
    NoEvent
}