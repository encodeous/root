use crate::concepts::neighbour::Neighbour;
use crate::framework::RoutingSystem;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::router::INF;

#[derive(Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Interface<T: RoutingSystem + ?Sized> {
    pub id: T::InterfaceId,
    pub net_type: T::NetworkType,
    pub neighbours: HashMap<T::NodeAddress, Neighbour<T>>,
}

impl<T: RoutingSystem + ?Sized> Interface<T> {
    fn new(id: T::InterfaceId, network_type: T::NetworkType) -> Self{
        Interface{
            net_type: network_type,
            id,
            neighbours: HashMap::new()
        }
    }
    fn add_neighbour(&mut self, addr: &T::NodeAddress, phy: &T::PhysicalAddress) {
        if !self.neighbours.contains_key(&addr){
            self.neighbours.insert(addr.clone(), Neighbour{
                itf: self.id.clone(),
                addr_phy: phy.clone(),
                addr: addr.clone(),
                routes: HashMap::new(),
                link_cost: INF
            });
        }
    }
    fn get_neighbour(&mut self, addr: &T::NodeAddress) -> Option<&mut Neighbour<T>>{
        self.neighbours.get_mut(addr)
    }
    fn remove_neighbour(&mut self, addr: &T::NodeAddress){
        self.neighbours.remove(addr);
    }
}
