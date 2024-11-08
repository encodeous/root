use root::router::INF;

mod common;

#[test]
fn retraction_link_down(){
    let mut network = common::graphs::vnet_simple_weighted();
    network.tick_n(10); // just make it converge

    // make link untraversable
    network.update_edge(3, INF); // should retract
    network.tick_n(2);
    assert_eq!(network.get_metric_to("1", "5"), INF);
    assert_eq!(network.get_next_hop("1", "5"), "2");

    network.tick_n(3); // wait for starvation to be fixed
    assert_eq!(network.get_metric_to("1", "5"), 9);
    assert_eq!(network.get_next_hop("1", "5"), "3");
}


#[test]
fn link_recovery_inf_sum(){
    let mut network = common::graphs::vnet_fragile_network();
    network.tick_n(10); // just make it converge

    assert_eq!(network.get_metric_to("3", "5"), 12);
    assert_eq!(network.get_next_hop("3", "5"), "1");
    
    // make link infeasible
    network.update_edge(3, 11);
    network.tick_n(2);
    assert_eq!(network.get_metric_to("3", "5"), INF);
    assert_eq!(network.get_next_hop("3", "5"), "1");

    network.tick_n(4); // wait for starvation to be fixed
    assert_eq!(network.get_metric_to("3", "5"), 13);
    assert_eq!(network.get_next_hop("3", "5"), "1");

    network.update_edge(3, INF-1);
    network.tick_n(6);
    assert_eq!(network.get_metric_to("3", "5"), INF-1); // should never be INF, since that is retracted
}