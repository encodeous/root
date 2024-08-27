use std::hash::Hash;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::router::Router;

pub trait RoutingSystem {
    /// Address of the node on the routing network, MUST be globally unique
    type NodeAddress: Ord + PartialOrd + RootData + RootKey;
    /// A type that describes a physical interface or higher level concept that allows this node to talk to another node via some method
    /// Must be unique
    type Link: RootKey + RootData;
    /// An opaque implementation that allows the node to sign packets
    type MACSystem: MACSystem<Self>;
}

pub trait RootData: Clone + Serialize + DeserializeOwned + Sized {}
pub trait RootKey: Eq + PartialEq + Hash {}
impl<T: Eq + PartialEq + Hash> RootKey for T {}
impl<T: Clone + Serialize + DeserializeOwned + Sized> RootData for T {}

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

