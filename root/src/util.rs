use crate::router::INF;
use std::cmp::min;

/// Compares whether a < b mod 65536
///
/// # Arguments
///
/// * `a`: First one
/// * `b`: Second one
///
/// returns: bool
///
/// # Examples
///
/// ```
/// let a = 5;
/// let b = 10000;
/// assert!(root::util::seqno_less_than(5,10000));
/// assert!(root::util::seqno_less_than(60000,61000));
///
/// assert!(!root::util::seqno_less_than(20000,61000));
/// ```
pub fn seqno_less_than(a: u16, b: u16) -> bool {
    let x = b.overflowing_sub(a).0 as i32 % 65536;
    0 < x && x < 32768
}

/// Shortcut for increment mod 2^16
pub fn increment(x: &mut u16) {
    *x = x.overflowing_add(1).0
}
pub fn increment_by(x: u16, y: u16) -> u16 {
    x.overflowing_add(y).0
}

pub fn sum_inf(cost_a: u16, cost_b: u16) -> u16 {
    if cost_a == INF || cost_b == INF {
        INF
    } else {
        min((INF - 1) as u32, cost_a as u32 + cost_b as u32) as u16
    }
}
