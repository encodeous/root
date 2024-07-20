use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::ptr::hash;
use std::str::FromStr;
use anyhow::{Context, Error};
use root::router::{INF, Router};
use crate::{DummyMAC, GraphInterface, GraphSystem, PAddr};
use yaml_rust2::{YamlLoader, YamlEmitter, Yaml};
use yaml_rust2::yaml::Hash;
use root::concepts::packet::{Packet, RouteUpdate};
use root::concepts::route::Route;
use root::framework::MACSystem;
use linear_map::LinearMap;

pub struct Graph {
    pub adj: Vec<(u8, u8, u16)>,
}

pub struct State {
    pub nodes: Vec<GraphSystem>,
    pub packets: BTreeMap<u8, Vec<(DummyMAC<Packet<GraphSystem>>, u8)>>,
    pub config: BTreeMap<String, bool>
}

pub fn serialize_packets() {}

pub fn serialize_route_update(update: &RouteUpdate<GraphSystem>) -> Yaml {
    let x = &update.source.data;
    Yaml::from_str(format!("{} {} {}", x.0, x.1, update.metric).as_str())
}

pub fn parse_route_update(yaml: &Yaml) -> anyhow::Result<RouteUpdate<GraphSystem>> {
    let parts = yaml.as_str().context("Expected route update string")?.split(' ').collect::<Vec<&str>>();

    Ok(
        RouteUpdate {
            metric: u16::from_str(parts[2]).unwrap(),
            source: DummyMAC::<(u8, u16)> {
                data: (
                    u8::from_str(parts[0]).unwrap(),
                    u16::from_str(parts[1]).unwrap()
                )
            },
        }
    )
}

pub fn serialize_route(rt: &Route<GraphSystem>, cur_routes: &mut Vec<Yaml>){
    let (src, seq) = &rt.source.data;

    if let Some(next_hop) = rt.next_hop {
        // not a self-route
        cur_routes.push(
            Yaml::from_str(format!("{src} {} {seq} {} {}", next_hop, rt.metric, rt.fd.unwrap_or(INF)).as_str())
        );
    } else {
        cur_routes.push(
            Yaml::from_str(format!("{src} - {seq} self").as_str())
        );
    }
}

pub fn parse_route(route: &str) -> anyhow::Result<Route<GraphSystem>> {
    let values: Vec<&str> = route.split_whitespace().collect();
    let source = DummyMAC{
        data: (
            u8::from_str(values[0])?,
            u16::from_str(values[2])?
        )
    };
    if !route.ends_with("self"){
        let next_hop = u8::from_str(values[1])?;
        let metric = u16::from_str(values[3])?;
        let fd = u16::from_str(values[4])?;
        Ok(
            Route{
                source,
                metric,
                next_hop: Some(next_hop),
                fd: if fd == INF {
                    None
                } else {
                    Some(fd)
                },
                itf: Some(1)
            }
        )
    }
    else{
        // self route
        Ok(
            Route{
                source,
                metric: 0,
                next_hop: None,
                fd: None,
                itf: None
            }
        )
    }
}

pub fn load(state: &Yaml) -> anyhow::Result<State> {
    let mut nodes: Vec<GraphSystem> = Vec::new();
    let mut packets: BTreeMap::<u8, Vec<(DummyMAC<Packet<GraphSystem>>, u8)>> = BTreeMap::new();

    let mut node_ids = BTreeSet::<u8>::new();
    let mut adj: Vec<(u8, u8, u16)> = Vec::new();

    for (k, v) in state["nodes"].as_hash().context("Expected map")?{
        let addr = k.as_i64().context("Expected int key")? as u8;
        node_ids.insert(addr);
        for pkt in v["packets"].as_vec().context("Expected array")?{
            let mp = pkt.as_hash().context("Expected map")?;
            let from = mp.get(&Yaml::from_str("from")).context("Expected from key")?.as_i64().context("Expected int from")? as u8;
            for (k, v) in mp {
                let p_type = k.as_str().context("Expected string key")?;
                match p_type {
                    "ru" => {
                        let values = packets.entry(addr).or_insert(vec![]);
                        values.push(
                            (
                                DummyMAC{
                                    data: Packet::RouteUpdate(
                                        parse_route_update(v)?
                                    )
                                },
                                from
                            )
                        );
                    },
                    "bru" => {
                        let mut val: Vec<RouteUpdate<GraphSystem>> = Vec::new();
                        for udt in v.as_vec().context("Expected list of updates")?{
                            val.push(parse_route_update(udt)?)
                        }
                        let values = packets.entry(addr).or_insert(vec![]);
                        values.push(
                            (
                                DummyMAC{
                                    data: Packet::BatchRouteUpdate{
                                        routes: val
                                    }
                                },
                                from
                            )
                        );
                    }
                    "from" => {

                    }
                    _ => {
                        return Err(Error::msg(format!("Unmatched type {p_type}")))
                    }
                }
            }
        }
    }

    for line in state["neighbours"].as_vec().context("Expected list of neighbours")?{
        let line = line.as_str().context("Expected edge str")?;

        let values: Vec<u32> = line.split_whitespace().map(|x| { x.parse::<u32>().unwrap() }).collect();

        let a = values[0] as u8;
        let b = values[1] as u8;
        let cost = values[2] as u16;
        adj.push((a, b, cost));
        adj.push((b, a, cost));
        node_ids.insert(a);
        node_ids.insert(b);
    }

    let routes = state["routes"].as_hash().context("Expected route table")?;

    for node in node_ids {
        let mut neigh = HashMap::<u8, u16>::new();
        for entry in adj.iter().filter(|x| { x.0 == node }) {
            neigh.insert(entry.1, entry.2);
        }
        let itf = GraphInterface {
            neigh,
            id: node,
        };

        let mut sys = GraphSystem {
            router: Router::new(node)
        };

        sys.router.init();
        sys.router.add_interface(Box::new(itf));
        sys.router.refresh_interfaces();


        for route in routes.get(&Yaml::Integer(node as i64)).context("Expected route entry for node")?.as_vec().context("Expected list of routes")?{
            let route_str = route.as_str().context("Expected route string")?;
            let route_parsed = parse_route(route_str)?;
            sys.router.routes.insert(
                route_parsed.source.data.0,
                route_parsed
            );
        }

        nodes.push(sys);
    }
    
    let mut config = BTreeMap::new();
    
    for (k, v) in state["config"].as_hash().context("Expected config map")?{
        config.insert(
            k.as_str().context("Expected config key")?.to_string(),
            v.as_bool().context("Expected true/false param")?
        );
    }

    Ok(
        State {
            packets,
            nodes,
            config
        }
    )
}

pub fn save(state: &State) -> Yaml {
    let mut pairs = BTreeSet::new();
    let mut neighbours = Vec::new();
    let mut nodes = Hash::new();
    let mut routes = Hash::new();

    for node in &state.nodes {
        let mut y_node = Hash::new();

        let addr = node.router.address;
        let mut packets = Vec::new();

        for (pkt, from) in state.packets.get(&addr).unwrap_or(&Vec::new()) {
            let mut pkt_map = Hash::new();
            pkt_map.insert(
                Yaml::from_str("from"),
                Yaml::Integer(*from as i64),
            );
            match &pkt.data {
                Packet::RouteUpdate(update) => {
                    pkt_map.insert(
                        Yaml::from_str("ru"),
                        serialize_route_update(update),
                    );
                }
                Packet::BatchRouteUpdate { routes } => {
                    let mut batch = routes.iter().map(serialize_route_update).collect::<Vec<Yaml>>();
                    batch.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    pkt_map.insert(
                        Yaml::from_str("bru"),
                        Yaml::Array(batch),
                    );
                }
                Packet::RouteRequest { .. } => {}
            }
            packets.push(Yaml::Hash(pkt_map));
        }
        packets.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        y_node.insert(Yaml::from_str("packets"), Yaml::Array(packets));

        nodes.insert(
            Yaml::Integer(node.router.address as i64),
            Yaml::Hash(y_node),
        );


        // calculate neighbours
        for (_, itf) in &node.router.interfaces {
            for (n_addr, _) in &itf.neighbours {
                if !pairs.contains(&(addr, *n_addr)) {
                    pairs.insert((addr, *n_addr));
                    pairs.insert((*n_addr, addr));
                    neighbours.push(Yaml::from_str(format!("{} {} {}", addr, *n_addr, itf.net_if.get_cost(&PAddr::GraphNode(*n_addr))).as_str()));
                }
            }
        }

        let mut cur_routes = Vec::new();
        for (src, rt) in &node.router.routes {
            serialize_route(rt, &mut cur_routes);
        }
        cur_routes.sort_by(|a, b| a.partial_cmp(b).unwrap());
        routes.insert(
            Yaml::Integer(node.router.address as i64),
            Yaml::Array(cur_routes),
        );
    }

    neighbours.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    
    let mut config = Hash::new();
    
    
    for (k, v) in &state.config{
        config.insert(
            Yaml::from_str(k.as_str()),
            Yaml::Boolean(*v)
        );
    }
    let mut root = Hash::new();
    root.insert(
        Yaml::from_str("config"),
        Yaml::Hash(config),
    );
    root.insert(
        Yaml::from_str("routes"),
        Yaml::Hash(routes),
    );
    root.insert(
        Yaml::from_str("neighbours"),
        Yaml::Array(neighbours),
    );
    root.insert(
        Yaml::from_str("nodes"),
        Yaml::Hash(nodes),
    );
    Yaml::Hash(root)
}