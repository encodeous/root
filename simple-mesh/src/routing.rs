use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::net::Ipv4Addr;
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use root::concepts::interface::{NetworkInterface};
use root::framework::{MACSystem, RoutingSystem};
use root::router::{INF, Router};
use crate::routing::NType::PhysicalIP;

pub struct IPV4System {
    pub(crate) router: Router<Self>
}

impl Clone for IPV4System {
    fn clone(&self) -> Self {
        todo!() // don't actually need to clone, rust's type system is too strict lol
    }
}

#[derive(Eq, PartialEq, Hash)]
pub enum NType {
    PhysicalIP,
}
impl RoutingSystem for IPV4System {
    type NodeAddress = u8;
    type PhysicalAddress = Ipv4Addr;
    type NetworkType = NType;
    type InterfaceId = u8;
    type MAC<T: Clone + Serialize + DeserializeOwned> = DummyMAC<T>;
    type DedupType = [u8; 16];
}

#[derive(Serialize, Deserialize)]
pub struct DummyMAC<T>
where
    T: Clone,
{
    pub data: T,
}

impl<V: Clone> Clone for DummyMAC<V> {
    fn clone(&self) -> Self {
        DummyMAC {
            data: self.data.clone(),
        }
    }
}

impl<V: Clone + Serialize + DeserializeOwned> MACSystem<V, IPV4System> for DummyMAC<V> {
    fn data(&self) -> &V {
        &self.data
    }

    fn data_mut(&mut self) -> &mut V {
        &mut self.data
    }

    fn validate(&self, subject: &u8) -> bool {
        true
    }

    fn sign(data: V, router: &Router<IPV4System>) -> DummyMAC<V> {
        DummyMAC::<V> { data }
    }
}

#[derive(Eq, PartialEq)]
struct Ipv4Interface {
    neigh: HashMap<u8, u16>,
    id: u8,
    self_addr: Ipv4Addr
}

impl NetworkInterface<IPV4System> for Ipv4Interface {
    fn address(&self) -> Ipv4Addr {
        todo!("Bruh")
    }

    fn address_type(&self) -> NType {
        PhysicalIP
    }

    fn id(&self) -> u8 {
        1
    }

    fn get_cost(&self, addr: &Ipv4Addr) -> u16 {
        INF
    }

    fn get_neighbours(&self) -> Vec<(Ipv4Addr, u8)> {
        todo!("Bruh")
    }
}