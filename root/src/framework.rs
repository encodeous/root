use std::hash::Hash;
use cfg_if::cfg_if;

cfg_if!{
    if #[cfg(feature = "serde")] {
        use serde::de::DeserializeOwned;
        use serde::Serialize;
    }
}

use crate::router::Router;

pub trait RoutingSystem {
    /// Maximal length that the warning log should be kept for, if the buffer is full, the oldest warning is dropped.
    const MAX_WARN_LENGTH: usize = 1000;
    /// Should the routing client trust seqno requests where the seqno > cur_seqno + 1. ENSURE MAC IS ENABLED
    const TRUST_RESYNC_SEQNO: bool = true;
    /// Address of the node on the routing network, MUST be globally unique
    type NodeAddress: RootData + RootKey;
    /// A type that describes a physical interface or higher level concept that allows this node to talk to another node via some method
    /// Must be unique
    type Link: RootData + RootKey;
    /// An opaque implementation that allows the node to sign packets
    type MACSystem: MACSystem<Self>;
}

cfg_if!{
    if #[cfg(feature = "serde")] {
        pub trait RootData: Clone + Serialize + DeserializeOwned + Sized {}
        impl<T: Clone + Serialize + DeserializeOwned + Sized> RootData for T {}
    }
    else{
        pub trait RootData: Clone + Sized {}
        impl<T: Clone + Sized> RootData for T {}
    }
}
pub trait RootKey: Eq + PartialEq + Hash {}
impl<T: Eq + PartialEq + Hash> RootKey for T {}

pub trait MACSignature<V: RootData, T: RoutingSystem + ?Sized>: RootData
{
    fn data(&self) -> &V;
    fn data_mut(&mut self) -> &mut V;
}

pub trait MACSystem<T: RoutingSystem + ?Sized>: Default {
    type MACSignatureType<V: RootData>: MACSignature<V, T>;
    fn sign<V: RootData>(&self, data: V, router: &Router<T>) -> Self::MACSignatureType<V>;
    fn validate<V: RootData>(&self, sig: &MAC<V, T>, subject: &T::NodeAddress) -> bool;
}
pub type MAC<V, T> = <<T as RoutingSystem>::MACSystem as MACSystem<T>>::MACSignatureType<V>;

