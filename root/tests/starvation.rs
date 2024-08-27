use root::router::INF;

mod common;

#[test]
fn seqno_request(){
    let mut network = common::graphs::vnet_simple_weighted();
    network.tick_n(10); // just make it converge

    // lower the metric
    network.update_edge(4, 1);
    network.tick_n(2);
    assert_eq!(network.get_metric_to("1", "5"), 3);
    assert_eq!(network.get_next_hop("1", "5"), "3");
    
    // increase the metric
    network.update_edge(4, 2);
    network.tick_n(2);
    assert_eq!(network.get_metric_to("1", "5"), INF);
    assert_eq!(network.get_next_hop("1", "5"), "3");
    
    // the starvation should be solved
    network.tick_n(3); // takes 3 ticks for the packet to travel 
    assert_eq!(network.get_metric_to("1", "5"), 4);
    assert_eq!(network.get_next_hop("1", "5"), "3");
}