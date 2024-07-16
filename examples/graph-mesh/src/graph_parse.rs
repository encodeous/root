use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use root::router::Router;
use crate::{GraphInterface, GraphSystem, PAddr};

pub struct Graph {
    pub adj: Vec<(u8, u8, u16)>
}

pub fn build<'s>(input: &'s str) -> Vec<GraphSystem>{
    let mut nodes: Vec<GraphSystem> = Vec::new();

    let mut graph: Graph = Graph{
        adj: vec![]
    };
    
    let mut node_ids = HashSet::<u8>::new();

    for line in input.split('\n'){
        let values: Vec<u32> = line.split_whitespace().map(|x| {x.parse::<u32>().unwrap()}).collect();

        let a = values[0] as u8;
        let b = values[1] as u8;
        let cost = values[2] as u16;
        graph.adj.push((a, b, cost));
        graph.adj.push((b, a, cost));
        node_ids.insert(a);
    }
    
    for node in node_ids{
        let mut neigh = HashMap::<u8, u16>::new();
        for entry in graph.adj.iter().filter(|x| {x.0 == node}){
            neigh.insert(entry.1, entry.2);
        }
        let itf = GraphInterface{
            neigh,
            id: node
        };
        
        let mut sys = GraphSystem::<'s>{
            router: Router::new(node)
        };
        
        sys.router.add_interface(Box::new(itf));
        
        nodes.push(sys);
    }

    nodes
}

pub fn state(networks: &[GraphSystem]) -> Vec<(u8, u8, String)>{
    let mut edges = Vec::<(u8, u8, String)>::new();
    for net in networks {
        for route in &net.router.routes{
            edges.push((net.router.address, *route.0, format!("c={},nh={}", route.1.metric, route.1.next_hop.unwrap())))
        }
    }
    edges
}