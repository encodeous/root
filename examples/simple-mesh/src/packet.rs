use serde::{Deserialize, Serialize};
use root::concepts::packet::Packet;
use crate::routing::IPV4System;

#[derive(Serialize, Deserialize)]
pub enum NetPacket{
    Ping(u32),
    Pong(u32),
    Routing {
        link_id: u32,
        data: Packet<IPV4System>
    }
}