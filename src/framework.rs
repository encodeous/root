use std::cell::RefCell;
use std::cmp::Ordering;
use std::hash::Hash;
use std::rc::Rc;
use std::time::Duration;
use crate::concepts::interface::{Interface, AddressType, NetworkInterface};
use crate::concepts::tlv::Tlv;

// pub trait SystemNetwork: Sized {
//     type NetworkTypes: Sized + TryFrom<u8> + Into<u8>;
//     fn get_interfaces(&self) -> Vec<Box<dyn NetworkInterface<Self>>>;
// }
// 
// pub trait Routing {
//     type AddressType: Sized + Hash + Eq + PartialEq;
//     fn config() -> ProtocolParams {
//         Default::default()
//     }
// }

pub trait RoutingSystem: Clone {
    type NodeAddress: Sized + Hash + Eq + PartialEq + Ord + PartialOrd + Clone;
    type NetworkAddress: Sized + AddressType<Self> + Hash + Eq + PartialEq;
    type NetworkType: Sized + Hash + Eq + PartialEq;
    type InterfaceId: Eq + PartialEq;
    
    fn config() -> ProtocolParams {
        Default::default()
    }
    fn get_interfaces(&self) -> &[Box<dyn NetworkInterface<Self>>];
}

/// Appendix B. Protocol Parameters
pub struct ProtocolParams{
    pub hello_interval: Duration,
    pub link_cost: u8,
    pub ihu_interval: Duration,
    pub update_interval: Duration,
    pub ihu_hold_time: Duration,
    pub route_expiry_time: Duration,
    pub base_request_timeout: Duration,
    pub urgent_timeout: Duration,
    pub source_gc_time: Duration,
}
impl Default for ProtocolParams{
    fn default() -> Self {
        // sensible defaults from Appendix B. Protocol Parameters
        let hello = 4000;
        Self{
            hello_interval: Duration::from_millis(hello),
            link_cost: 96,
            ihu_interval: Duration::from_millis(hello * 3),
            update_interval: Duration::from_millis(hello * 4),
            ihu_hold_time: Duration::from_millis((hello as f64 * 3.0 * 3.5) as u64),
            route_expiry_time: Duration::from_millis((hello as f64 * 4.0 * 3.5) as u64),
            base_request_timeout: Duration::from_millis(2000),
            urgent_timeout: Duration::from_millis(200),
            source_gc_time: Duration::from_secs(60 * 3),
        }
    }
}