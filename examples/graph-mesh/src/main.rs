use std::collections::HashMap;
use std::sync::Mutex;
use root::concepts::interface::{AddressType, NetworkInterface};
use root::framework::{MACSystem, RoutingSystem};
use root::router::{INF, Router};
use crate::graph_parse::build;
use crate::PAddr::GraphNode;
use crate::NType::GraphT1;

mod graph_parse;

struct GraphSystem<'system>{
    router: Router<'system, Self>
}

impl<'s> Clone for GraphSystem<'s>{
    fn clone(&self) -> Self {
        todo!() // don't actually need to clone, rust's type system is too strict
    }
}

#[derive(Eq, PartialEq, Hash)]
enum PAddr {
    GraphNode(u8)
}

#[derive(Eq, PartialEq, Hash)]
enum NType {
    GraphT1
}

impl<'s> AddressType<GraphSystem<'s>> for PAddr {
    fn get_network_type(&self) -> NType {
        match self {
            PAddr::GraphNode(_) => {
                GraphT1
            }
        }
    }
}

impl<'s> RoutingSystem for GraphSystem<'s>{
    type NodeAddress = u8;
    type PhysicalAddress = PAddr;
    type NetworkType = NType;
    type InterfaceId = u8;
    type MAC<T> = DummyMAC<T>;
}

struct DummyMAC<T>{
    pub data: T
}

impl<'s, T> MACSystem<GraphSystem<'s>> for DummyMAC<T> {
    
}

#[derive(Eq, PartialEq)]
struct GraphInterface {
    neigh: HashMap<u8, u16>,
    id: u8
}

impl<'s> NetworkInterface<GraphSystem<'s>> for GraphInterface{
    fn address(&self) -> PAddr {
        GraphNode(self.id)
    }

    fn address_type(&self) -> NType {
        GraphT1
    }

    fn id(&self) -> u8 {
        self.id
    }

    fn get_cost(&self, addr: &PAddr) -> u16 {
        if let GraphNode(id) = addr{
            return self.neigh[id]
        }
        INF
    }

    fn get_neighbours(&self) -> Vec<(PAddr, u8)> {
        let mut neighbours = Vec::new();
        for (addr, cost) in &self.neigh{
            neighbours.push((GraphNode(*addr), *addr))
        }
        neighbours
    }
}

fn main() {
    let mut nodes = build(r"1 2 2
2 3 4
3 4 100
4 5 1
1 3 1
3 5 8
4 2 5");
    
}
