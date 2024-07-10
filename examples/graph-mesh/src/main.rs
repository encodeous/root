use std::collections::HashMap;
use std::sync::Mutex;
use root::concepts::interface::{AddressType, NetworkInterface};
use root::framework::{RoutingSystem};
use root::router::INF;
use crate::NAddr::GraphNode;
use crate::NType::GraphT1;

mod graph_parse;

struct GraphSystem{
    interfaces: Vec<Box<dyn NetworkInterface<Self>>>
}

impl Clone for GraphSystem{
    fn clone(&self) -> Self {
        todo!() // don't actually need to clone, rust's type system is too strict
    }
}

#[derive(Eq, PartialEq, Hash)]
enum NAddr {
    GraphNode(u8)
}

#[derive(Eq, PartialEq, Hash)]
enum NType {
    GraphT1
}

impl AddressType<GraphSystem> for NAddr {
    fn get_network_type(&self) -> NType {
        match self {
            NAddr::GraphNode(_) => {
                GraphT1
            }
        }
    }
}

impl RoutingSystem for GraphSystem{
    type NodeAddress = u8;
    type NetworkAddress = NAddr;
    type NetworkType = NType;
    type InterfaceId = u8;

    fn get_interfaces(&self) -> &[Box<dyn NetworkInterface<Self>>] {
        self.interfaces.as_slice()
    }
}

#[derive(Eq, PartialEq)]
struct GraphInterface {
    neigh: HashMap<u8, u16>,
    id: u8
}

impl NetworkInterface<GraphSystem> for GraphInterface{
    fn address(&self) -> NAddr {
        GraphNode(self.id)
    }

    fn address_type(&self) -> NType {
        GraphT1
    }

    fn id(&self) -> u8 {
        self.id
    }

    fn get_cost(&self, addr: &NAddr) -> u16 {
        if let GraphNode(id) = addr{
            return self.neigh[id]
        }
        INF
    }
}

fn main() {
    println!("Hello, world!");
}
