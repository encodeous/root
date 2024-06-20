use root::concepts::interface::{NetworkAddress, NetworkInterface};
use root::framework::{SystemNetwork, Routing};
use root::router::Router;
use crate::NetworkTypes::UDP;

struct UDPAddress {
    
}

impl NetworkAddress<Network> for UDPAddress{
    fn network_type(&self) -> Network::NetworkTypes {
        UDP
    }

    fn get_bytes(&self) -> Vec<u8> {
        todo!()
    }
}

struct UDPInterface{
    
}

impl NetworkInterface<Network> for UDPInterface{
    fn network_type(&self) -> Network::NetworkTypes {
        UDP
    }

    fn address(&self) -> Box<dyn NetworkAddress<Network>> {
        todo!()
    }

    fn get_id(&self) -> Vec<u8> {
        vec![1]
    }

    fn get_cost(&self, addr: &dyn NetworkAddress<Network>) -> u16 {
        255
    }
}

struct Routing{

}

impl Routing for Routing{
    type AddressType = [u8; 16];
}

enum NetworkTypes {
    UDP = 0
}

struct Network{

}

impl SystemNetwork for Network{
    type NetworkTypes = NetworkTypes;

    fn get_interfaces(&self) -> Vec<Box<dyn NetworkInterface<Self>>> {
        todo!()
    }
}


fn main() {
    let router = Router::<Network, Routing>::new();

    let nt = UDP as u8;
}
