use std::cell::RefCell;
use std::cmp::Ordering;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::rc::Rc;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use crate::concepts::interface::{Interface, AddressType, NetworkInterface};
use crate::concepts::packet::Packet;
use crate::router::Router;
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
    /// Address of the node on the routing network, MUST be globally unique
    type NodeAddress: Sized + Hash + Eq + PartialEq + Ord + PartialOrd + Clone + Serialize + DeserializeOwned;
    /// Address of a node on the physical network, may not be globally unique, and may be overlapping
    type PhysicalAddress: Sized + AddressType<Self> + Hash + Eq + PartialEq + Serialize + DeserializeOwned;
    type NetworkType: Sized + Hash + Eq + PartialEq;
    type InterfaceId: Sized + Eq + PartialEq + Hash + Clone + Serialize + DeserializeOwned;
    type MAC<T: Clone + Serialize + DeserializeOwned>: MACSystem<T, Self>;
    /// type used for deduplication
    type DedupType: Sized + Hash + Eq + PartialEq + Ord + PartialOrd + Clone;
    fn config() -> ProtocolParams {
        Default::default()
    }
}

pub trait MACSystem<V: Clone + Serialize + DeserializeOwned, T: RoutingSystem>: Clone + Serialize + DeserializeOwned{
    fn data(&self) -> &V;
    fn data_mut(&mut self) -> &mut V;
    fn validate(&self, subject: &T::NodeAddress) -> bool;
    fn sign(data: V, router: &Router<T>) -> T::MAC<V>;
}

/// Appendix B. Protocol Parameters
pub struct ProtocolParams{
    pub dedup_ttl: Duration,
    pub cleanup_timer: Duration
}
impl Default for ProtocolParams{
    fn default() -> Self {
        Self{
            dedup_ttl: Duration::from_secs(60),
            cleanup_timer: Duration::from_secs(15),
        }
    }
}