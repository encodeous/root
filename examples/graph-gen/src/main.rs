use std::cmp::{max, min};
use std::collections::{HashMap, HashSet};
use rand::{Rng};

fn main() {
    const N: i32 = 50;
    const EDGES: i32 = 100;
    let mut edges = HashSet::new();

    while edges.len() < EDGES as usize {
        let a = rand::thread_rng().gen_range(1..N);
        let b = rand::thread_rng().gen_range(1..N);
        if a == b{
            continue;
        }
        let c = rand::thread_rng().gen_range(1..100);
        let (a, b) = (min(a, b), max(a, b));
        edges.insert((a,b,c));
    }
    
    for (a,b,c) in edges{
        println!("- {a} {b} {c}")
    }
}
