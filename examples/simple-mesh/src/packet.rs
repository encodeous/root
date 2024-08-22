use serde::{Deserialize, Serialize};
use uuid::Uuid;
use root::concepts::packet::Packet;
use root::framework::RoutingSystem;
use crate::routing::IPV4System;

#[derive(Serialize, Deserialize)]
pub enum NetPacket{
    Ping(<IPV4System as RoutingSystem>::Link, bool),
    Pong(<IPV4System as RoutingSystem>::Link, bool),
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
    }
}