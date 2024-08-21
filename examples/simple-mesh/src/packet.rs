use serde::{Deserialize, Serialize};
use uuid::Uuid;
use root::concepts::packet::Packet;
use crate::routing::IPV4System;

#[derive(Serialize, Deserialize)]
pub enum NetPacket{
    Ping(Uuid),
    Pong(Uuid),
    Routing {
        link_id: Uuid,
        data: Packet<IPV4System>
    }
}