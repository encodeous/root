use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;

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

pub trait RoutingSystem {
    /// Address of the node on the routing network, MUST be globally unique
    type NodeAddress: Ord + PartialOrd + RootData + RootKey;
    /// Address of a node on the physical network, may not be globally unique, and may be overlapping
    type PhysicalAddress: RootKey + RootData;
    type NetworkType: RootKey + RootData;
    type InterfaceId: RootKey + RootData;
    type MAC<T: RootData>: MACSystem<T, Self>;
    /// type used for deduplication
    type DedupType: Sized + Hash + Eq + PartialEq + Ord + PartialOrd + Clone;
    fn config() -> ProtocolParams {
        Default::default()
    }
}

pub trait RootData: Clone + Serialize + DeserializeOwned + Sized {}
pub trait RootKey: Eq + PartialEq + Hash {}
impl<T: Eq + PartialEq + Hash> RootKey for T {}
impl<T: Clone + Serialize + DeserializeOwned + Sized> RootData for T {}

pub trait MACSystem<V: RootData, T: RoutingSystem + ?Sized>: RootData
{
    fn data(&self) -> &V;
    fn data_mut(&mut self) -> &mut V;
    fn validate(&self, subject: &T::NodeAddress) -> bool;
    fn sign(data: V, router: &Router<T>) -> T::MAC<V>;
}

/// Appendix B. Protocol Parameters
pub struct ProtocolParams {
    pub dedup_ttl: Duration,
    pub cleanup_timer: Duration,
}
impl Default for ProtocolParams {
    fn default() -> Self {
        Self {
            dedup_ttl: Duration::from_secs(60),
            cleanup_timer: Duration::from_secs(15),
        }
    }
}
