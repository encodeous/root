/// Compares whether a < b
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
/// assert!(root::seqno::seqno_less_than(5,10000));
/// assert!(root::seqno::seqno_less_than(60000,61000));
/// 
/// assert!(!root::seqno::seqno_less_than(20000,61000));
/// ```
pub fn seqno_less_than(a: u16, b: u16) -> bool {
    let x = (b - a) as i32 % 65536;
    0 < x && x < 32768
}

/// Shortcut for increment mod 2^16
pub fn increment(x: &mut u16){
    *x = x.overflowing_add(1).0
}