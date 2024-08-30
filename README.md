# root

[root](https://github.com/encodeous/root) is an abstract I/O-free implementation of the [Babel Routing Protocol](https://datatracker.ietf.org/doc/html/rfc8966) with its own take. It provides a high-level framework to design dynamic, fault-tolerant networks.

The application is solely responsible for providing I/O and scheduling events, meaning that users of root are free to use any platform, framework, or architecture they desire.

For a complete example routing application that uses TCP as transport, see `/examples/simple-mesh`.

To get started with root, run:
`cargo add root`, use the `serde` feature for serialization.

# Why I/O-free?

root is designed from the ground up to offer a platform, network, and protocol agnostic way to do routing.
- Compared to traditional implementations that rely on a **specific network stack**, root is able to work for **any situation** where a graph of nodes are joined together with bidirectional links.
- Decisions about how to physically forward packets are left to the **application**, allowing hybrid routing over **multiple protocols** (i.e routing over IPv4, IPv6, Serial & Bluetooth all at once)
- An I/O-free implementation allows root to be thoroughly tested, and operate with deterministic state at all times.

For more motivations, you can read [this set of articles](https://sans-io.readthedocs.io/index.html#).


# Concepts

root tries its best to abstract the complexity of networking, while maintaining compatibility with low level concepts.

## Templating

When building a routing network using the root framework, the architect can specify a set of pre-defined parameters that defines it.

## The `NodeAddress` type

The NodeAddress is a globally (on each network) unique identifier that is attached to each node.

## The `Link` type

The link type represents a physical bidirectional connection between two nodes. This is not sent to other nodes, and should be unique on each node.

# Example Usage

> [!CAUTION]
> These examples do not implement MAC, meaning that routes/packets can be forged. The root crate implicitly trusts the authenticity of such packets. 

## Basic Example

To demonstrate the use of the root crate, here is a super simple example where we have 3 nodes, `bob`, `eve`, and `alice`.

We have: `bob <-> eve <-> alice`, but not `bob <-> alice`.

We want the routing system to figure out how to reach `alice` from `bob`

We can start off by defining the routing parameters. This is a compile-time constant shared across all nodes.

```rust
use root::framework::RoutingSystem;
use root::router::NoMACSystem;

struct SimpleExample {} // just a type to inform root of your network parameters
impl RoutingSystem for SimpleExample{
    type NodeAddress = String; // our nodes have string names
    type Link = i32;
    type MACSystem = NoMACSystem; // we won't use MAC for this example
}
```

Now, for each node, we can create a router:
```rust
// we have the following connection: bob <-> eve <-> alice

let mut nodes = HashMap::new();

let mut bob = Router::<SimpleExample>::new("bob".to_string());
bob.links.insert(1, Neighbour::new("eve".to_string()));
nodes.insert("bob", bob);

let mut eve = Router::<SimpleExample>::new("eve".to_string());
eve.links.insert(1, Neighbour::new("bob".to_string()));
eve.links.insert(2, Neighbour::new("alice".to_string()));
nodes.insert("eve", eve);

let mut alice = Router::<SimpleExample>::new("alice".to_string());
alice.links.insert(2, Neighbour::new("eve".to_string()));
nodes.insert("alice", alice);
```

Now we can let root take over, and have it automatically discover the route.

We simply let root generate routing packets, and simulate sending them to the other nodes. In a real network, these packets need to be serialized and sent over the network.

```rust
// lets simulate routing!

for step in 0..3 {
    // collect all of our packets, if any
    let packets: Vec<OutboundPacket<SimpleExample>> = nodes.iter_mut().flat_map(|(_id, node)| node.outbound_packets.drain(..)).collect();

    for OutboundPacket{link, dest, packet} in packets{
        // deliver the routing packet. in this simple example, the link isn't really used. in a real network, this link will give us information on how to send the packet
        if let Some(node) = nodes.get_mut(dest.as_str()){
            node.handle_packet(&packet, &link, &dest).expect("Failed to handle packet");
        }
    }

    for node in nodes.values_mut(){
        node.full_update(); // performs route table calculations, and writes routing updates into outbound_packets
    }

    // lets observe bob's route table:
    println!("Bob's routes in step {step}:");
    for (neigh, Route::<SimpleExample>{ metric, next_hop, .. }) in &nodes["bob"].routes{
        println!(" - {neigh}: metric: {metric}, next_hop: {next_hop}")
    }
}
```

Here is the output for this example:
```
Bob's routes in step 0:
Bob's routes in step 1:
- eve: metric: 1, next_hop: eve
Bob's routes in step 2:
- eve: metric: 1, next_hop: eve
    - alice: metric: 2, next_hop: eve
```
> [!NOTE]  
> You can try running this example yourself, its files are located in `./examples/super-simple`

## Network Example

> [!NOTE]  
> To demonstrate the root crate working over real network connections, a complete example is provided in `./examples/simple-mesh`.

This example uses TCP streams as the transport, and is based on a event/channel pattern.