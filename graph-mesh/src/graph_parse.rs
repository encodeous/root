use crate::{DummyMAC, GraphSystem, NType, PAddr};
use anyhow::{anyhow, ensure, Context, Error};
use linear_map::LinearMap;
use root::concepts::packet::{Packet, RouteUpdate};
use root::concepts::route::{Route, Source};
use root::framework::MACSignature;
use root::router::{Router, INF};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::ptr::hash;
use std::str::FromStr;
use yaml_rust2::yaml::Hash;
use yaml_rust2::{Yaml, YamlEmitter, YamlLoader};
use root::concepts::interface::Interface;
use root::concepts::neighbour::Neighbour;

pub struct Graph {
    pub adj: Vec<(u8, u8, u16)>,
}

pub struct State {
    pub nodes: Vec<GraphSystem>,

    /// map of [Dest, Vec<Packet, From>]
    pub packets: BTreeMap<u8, Vec<(DummyMAC<Packet<GraphSystem>>, u8)>>,
    pub config: BTreeMap<String, bool>,
    pub seq_requests: Vec<u8>,
}

pub fn serialize_update(update: &RouteUpdate<GraphSystem>) -> Yaml {
    let x = &update.source.data;
    Yaml::from_str(format!("{} {} {}", x.addr, x.seqno, update.metric).as_str())
}

pub fn parse_update(yaml: &Yaml) -> anyhow::Result<RouteUpdate<GraphSystem>> {
    let parts = yaml
        .as_str()
        .context("Expected seqno/route update string")?
        .split(' ')
        .collect::<Vec<&str>>();
    ensure!(parts.len() == 3, "Expected three elements in route");
    Ok(RouteUpdate {
        metric: u16::from_str(parts[2])?,
        source: DummyMAC {
            data: Source {
                addr: u8::from_str(parts[0])?,
                seqno: u16::from_str(parts[1])?,
            },
        },
    })
}

pub fn serialize_seqno_pair(source: u8, seqno: u16) -> Yaml {
    Yaml::from_str(format!("{} {}", source, seqno).as_str())
}

pub fn parse_seqno_pair(yaml: &Yaml) -> anyhow::Result<(u8, u16)> {
    let parts = yaml
        .as_str()
        .context("Expected seqno pair")?
        .split(' ')
        .collect::<Vec<&str>>();
    ensure!(parts.len() == 2, "Expected two elements in pair");
    Ok((u8::from_str(parts[0])?, u16::from_str(parts[1])?))
}

pub fn serialize_route(rt: &Route<GraphSystem>, cur_routes: &mut Vec<Yaml>) {
    let Source { addr, seqno } = rt.source.data;

    if let Some(next_hop) = rt.next_hop {
        // not a self-route
        cur_routes.push(Yaml::from_str(
            format!(
                "{addr} {} {seqno} {} {}",
                next_hop,
                rt.metric,
                rt.fd.unwrap_or(INF)
            )
            .as_str(),
        ));
    } else {
        cur_routes.push(Yaml::from_str(format!("{addr} - {seqno} self").as_str()));
    }
}

pub fn parse_route(route: &str) -> anyhow::Result<Route<GraphSystem>> {
    let values: Vec<&str> = route.split_whitespace().collect();
    let source = DummyMAC {
        data: Source {
            addr: u8::from_str(values[0])?,
            seqno: u16::from_str(values[2])?,
        },
    };
    if !route.ends_with("self") {
        ensure!(values.len() == 5, "Expected five elements in regular route");
        let next_hop = u8::from_str(values[1])?;
        let metric = u16::from_str(values[3])?;
        let fd = u16::from_str(values[4])?;
        Ok(Route {
            source,
            metric,
            next_hop: Some(next_hop),
            fd: if fd == INF { None } else { Some(fd) },
            itf: Some(1),
        })
    } else {
        // self route
        ensure!(values.len() == 4, "Expected four elements in self route");
        Ok(Route {
            source,
            metric: 0,
            next_hop: None,
            fd: None,
            itf: None,
        })
    }
}

pub fn load(state: &Yaml) -> anyhow::Result<State> {
    let mut nodes: Vec<GraphSystem> = Vec::new();
    let mut node_seqno: HashMap<u8, u16> = HashMap::new();
    let mut packets: BTreeMap<u8, Vec<(DummyMAC<Packet<GraphSystem>>, u8)>> = BTreeMap::new();
    let mut seqno_requests: BTreeMap<u8, Vec<(u8, u16)>> = BTreeMap::new();

    let mut node_ids = BTreeSet::<u8>::new();
    let mut adj: Vec<(u8, u8, u16)> = Vec::new();
    let mut seq_requests: Vec<u8> = Vec::new();
    let mut config = BTreeMap::new();

    if let Some(root) = state.as_hash() {
        if root.get(&Yaml::from_str("nodes")).is_some() {
            for (k, v) in state["nodes"].as_hash().context("Expected map")? {
                let addr = u8::from_str(k.as_str().context("Expected node key")?)?;
                node_ids.insert(addr);
                for pkt in v["packets"].as_vec().context("Expected array")? {
                    let mp = pkt.as_hash().context("Expected map")?;
                    let from = u8::from_str(
                        mp.get(&Yaml::from_str("from"))
                            .context("Expected from key")?
                            .as_str()
                            .context("Expected string from")?,
                    )?;
                    for (k, v) in mp {
                        let p_type = k.as_str().context("Expected string key")?;
                        match p_type {
                            // r == w, as we all know.
                            "uwu" => {
                                let values = packets.entry(addr).or_default();
                                values.push((
                                    DummyMAC {
                                        data: Packet::UrgentRouteUpdate(parse_update(v)?),
                                    },
                                    from,
                                ));
                            }
                            "bru" => {
                                let mut val: Vec<RouteUpdate<GraphSystem>> = Vec::new();
                                for udt in v.as_vec().context("Expected list of updates")? {
                                    val.push(parse_update(udt)?)
                                }
                                let values = packets.entry(addr).or_default();
                                values.push((
                                    DummyMAC {
                                        data: Packet::BatchRouteUpdate { routes: val },
                                    },
                                    from,
                                ));
                            }
                            "seqr" => {
                                let values = packets.entry(addr).or_default();
                                let pair = parse_seqno_pair(v)?;
                                values.push((
                                    DummyMAC {
                                        data: Packet::SeqnoRequest {
                                            source: pair.0,
                                            seqno: pair.1,
                                        },
                                    },
                                    from,
                                ))
                            }
                            "from" => {
                                // dont really care
                            }
                            _ => return Err(Error::msg(format!("Unmatched type {p_type}"))),
                        }
                    }
                }

                for pair in v["seqno-requests"].as_vec().context("Expected array")? {
                    let values = seqno_requests.entry(addr).or_default();
                    values.push(parse_seqno_pair(pair)?)
                }

                node_seqno.insert(addr, v["seqno"].as_i64().unwrap_or(0) as u16);
            }
        }

        for line in state["neighbours"]
            .as_vec()
            .context("Expected list of neighbours")?
        {
            let line = line.as_str().context("Expected edge str")?;

            let mut values = Vec::new();
            for val in line.split_whitespace() {
                values.push(val.parse::<u32>().context("Numbers must be positive")?)
            }

            let a = values[0] as u8;
            let b = values[1] as u8;
            let cost = values[2] as u16;
            adj.push((a, b, cost));
            adj.push((b, a, cost));
            node_ids.insert(a);
            node_ids.insert(b);
        }
        
        
        // create the nodes
        for node in &node_ids {
            let mut neighbours = HashMap::new();
            for (_, neigh, metric) in adj.iter().filter(|x| x.0 == *node) {
                neighbours.insert(
                    *neigh,
                    Neighbour{
                        link_cost: *metric,
                        addr: *neigh,
                        routes: HashMap::new(),
                        addr_phy: PAddr::GraphNode(*neigh),
                        itf: 1
                    }
                );
            }
            let mut sys = GraphSystem {
                router: Router::new(*node),
            };
            sys.router.interfaces.insert(1, Interface{
                net_type: NType::GraphT1,
                id: 1,
                neighbours
            });
            nodes.push(sys);
        }

        // restore the nodes routes
        if root.get(&Yaml::from_str("routes")).is_some() {
            let routes = state["routes"].as_hash().context("Expected route table")?;

            for node in &mut nodes {
                let node_id = node.router.address;
                for route in routes
                    .get(&Yaml::String(node_id.to_string()))
                    .context("Expected route entry for node")?
                    .as_vec()
                    .context("Expected list of routes")?
                {
                    let route_str = route.as_str().context("Expected route string")?;
                    let route_parsed = parse_route(route_str)?;
                    node.router
                        .routes
                        .insert(route_parsed.source.data.addr, route_parsed);
                }
                
            }
        }

        // create the nodes
        for sys in &mut nodes{
            let node = sys.router.address;

            if seqno_requests.contains_key(&node) {
                for (k, v) in &seqno_requests[&node] {
                    sys.router.seqno_requests.insert(*k, *v);
                }
            }
        }

        for (k, v) in state["config"].as_hash().context("Expected config map")? {
            config.insert(
                k.as_str().context("Expected config key")?.to_string(),
                v.as_bool().context("Expected true/false param")?,
            );
        }

        // actions that the front end sends to us:

        if let Some(actions) = root.get(&Yaml::from_str("actions")) {
            for (k, v) in actions.as_hash().context("Expected map of actions")? {
                match k.as_str().context("Expected string action key")? {
                    "req" => {
                        for cur_node in v.as_vec().context("Expected list")? {
                            let id = u8::from_str(cur_node.as_str().context("Expected node id")?)?;
                            if let Some(node) = nodes.iter_mut().find(|x| x.router.address == id) {
                                seq_requests.push(id);
                            }
                        }
                    }
                    "" => {}
                    _ => {}
                }
            }
        }
    }

    Ok(State {
        packets,
        nodes,
        config,
        seq_requests,
    })
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

        let mut node_requests = Vec::new();

        for (k, v) in &node.router.seqno_requests {
            node_requests.push(serialize_seqno_pair(*k, *v))
        }
        y_node.insert(Yaml::from_str("seqno-requests"), Yaml::Array(node_requests));
        y_node.insert(Yaml::from_str("seqno"), Yaml::Integer(node.router.seqno as i64));

        for (pkt, from) in state.packets.get(&addr).unwrap_or(&Vec::new()) {
            let mut pkt_map = Hash::new();
            pkt_map.insert(Yaml::from_str("from"), Yaml::String(from.to_string()));
            match &pkt.data {
                Packet::UrgentRouteUpdate(update) => {
                    pkt_map.insert(Yaml::from_str("uwu"), serialize_update(update));
                }
                Packet::BatchRouteUpdate { routes } => {
                    let mut batch = routes.iter().map(serialize_update).collect::<Vec<Yaml>>();
                    batch.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    pkt_map.insert(Yaml::from_str("bru"), Yaml::Array(batch));
                }
                Packet::SeqnoRequest { source, seqno } => {
                    pkt_map.insert(
                        Yaml::from_str("seqr"),
                        serialize_seqno_pair(*source, *seqno),
                    );
                }
            }
            packets.push(Yaml::Hash(pkt_map));
        }
        packets.sort_by(|a, b| a.partial_cmp(b).unwrap());

        y_node.insert(Yaml::from_str("packets"), Yaml::Array(packets));

        nodes.insert(
            Yaml::String(node.router.address.to_string()),
            Yaml::Hash(y_node),
        );

        // calculate neighbours
        for (_, itf) in &node.router.interfaces {
            for (n_addr, neigh) in &itf.neighbours {
                if !pairs.contains(&(addr, *n_addr)) {
                    pairs.insert((addr, *n_addr));
                    pairs.insert((*n_addr, addr));
                    neighbours.push(Yaml::from_str(
                        format!(
                            "{} {} {}",
                            addr,
                            *n_addr,
                            neigh.link_cost
                        )
                        .as_str(),
                    ));
                }
            }
        }

        let mut cur_routes = Vec::new();
        for (src, rt) in &node.router.routes {
            serialize_route(rt, &mut cur_routes);
        }
        cur_routes.sort_by(|a, b| a.partial_cmp(b).unwrap());
        routes.insert(
            Yaml::String(node.router.address.to_string()),
            Yaml::Array(cur_routes),
        );
    }

    neighbours.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mut config = Hash::new();

    for (k, v) in &state.config {
        config.insert(Yaml::from_str(k.as_str()), Yaml::Boolean(*v));
    }
    let mut root = Hash::new();
    root.insert(Yaml::from_str("config"), Yaml::Hash(config));
    root.insert(Yaml::from_str("neighbours"), Yaml::Array(neighbours));
    root.insert(Yaml::from_str("routes"), Yaml::Hash(routes));
    root.insert(Yaml::from_str("nodes"), Yaml::Hash(nodes));
    Yaml::Hash(root)
}
