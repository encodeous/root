use crate::common::virtual_network::VirtualSystem;

pub fn vnet_simple_weighted() -> VirtualSystem{
    VirtualSystem::create(
        &["1", "2", "3", "4", "5"],
        &[
            (0, "1", "2", 2),
            (1, "1", "3", 1),
            (2, "2", "3", 4),
            (3, "2", "4", 5),
            (4, "3", "4", 100),
            (5, "3", "5", 8),
            (6, "4", "5", 1),
        ]
    )
}

pub fn vnet_fragile_network() -> VirtualSystem{
    VirtualSystem::create(
        &["1", "2", "3", "4", "5"],
        &[
            (0, "1", "2", 1),
            (1, "1", "3", 1),
            (2, "2", "3", 1),
            (3, "1", "4", 10),
            (4, "5", "4", 1),
            (5, "6", "4", 1),
            (6, "6", "5", 1),
        ]
    )
}