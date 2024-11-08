
mod common;

#[test]
fn simple_weighted_graph(){
    let mut network = common::graphs::vnet_simple_weighted();
    network.tick_n(10); // just make it converge
    
    // at node 1
    assert_eq!(network.get_next_hop("1", "5"), "2");
    assert_eq!(network.get_metric_to("1", "5"), 8);
    assert_eq!(network.get_next_hop("1", "3"), "3");
    
    // at node 3
    assert_eq!(network.get_next_hop("3", "4"), "1");
    assert_eq!(network.get_metric_to("3", "4"), 8);
}

#[test]
fn route_optimizer(){
    let mut network = common::graphs::vnet_simple_weighted();
    network.tick_n(10); // just make it converge

    // improve the link between 3 and 5
    network.update_edge(5, 1);
    
    network.tick_n(2);
    
    // at node 1
    assert_eq!(network.get_next_hop("1", "5"), "3");
    assert_eq!(network.get_metric_to("1", "5"), 2);
}