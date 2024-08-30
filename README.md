# root

[root](https://github.com/encodeous/root) is an abstract I/O-free implementation of the [Babel Routing Protocol](https://datatracker.ietf.org/doc/html/rfc8966) with its own take. It provides a high-level framework to design dynamic, fault-tolerant networks.

The application is solely responsible for providing I/O and scheduling events, meaning that users of root are free to use any platform, framework, or architecture they desire.

For a complete example routing application that uses TCP as transport, see `/examples/simple-mesh`.

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

To create a bare-bones network using root, you can use the following example:

```rust
struct SimpleExample {} // just a type to inform root of your network parameters
impl RoutingSystem for SimpleExample{
    type NodeAddress = String; // our nodes have string names
    type Link = i32;
    type MACSystem = NoMACSystem; // we won't use MAC for this example
}
```

