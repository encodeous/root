use std::time::Instant;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use root::concepts::packet::Packet;
use root::framework::RoutingSystem;
use crate::routing::IPV4System;

#[derive(Serialize, Deserialize)]
pub enum NetPacket{
    Ping(<IPV4System as RoutingSystem>::Link),
    Pong(<IPV4System as RoutingSystem>::Link),
    Routing {
        link_id: <IPV4System as RoutingSystem>::Link,
        data: Packet<IPV4System>
    },
    LinkRequest{
        link_id: <IPV4System as RoutingSystem>::Link,
        from: <IPV4System as RoutingSystem>::NodeAddress
    },
    LinkResponse{
        link_id: <IPV4System as RoutingSystem>::Link,
        node_id: <IPV4System as RoutingSystem>::NodeAddress
    },
    Deliver{
        dst_id: <IPV4System as RoutingSystem>::NodeAddress,
        sender_id: <IPV4System as RoutingSystem>::NodeAddress,
        data: RoutedPacket
    },
    TraceRoute{
        dst_id: <IPV4System as RoutingSystem>::NodeAddress,
        sender_id: <IPV4System as RoutingSystem>::NodeAddress,
        path: Vec<<IPV4System as RoutingSystem>::NodeAddress>
    }
}

#[derive(Serialize, Deserialize)]
pub enum RoutedPacket{
    Ping,
    Pong,
    TracedRoute{
        path: Vec<<IPV4System as RoutingSystem>::NodeAddress>
    },
    Message(String),
    Undeliverable{
        to: <IPV4System as RoutingSystem>::NodeAddress
    }
}