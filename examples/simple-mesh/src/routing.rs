use root::framework::RoutingSystem;
use root::router::NoMACSystem;

pub struct IPV4System {}
impl RoutingSystem for IPV4System {
    type NodeAddress = String;
    type Link = u32;
    type MACSystem = NoMACSystem;
}
