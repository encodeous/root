pub mod concepts;
pub mod framework;
pub mod router;
pub mod util;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

trait Address {
    const BYTES: usize;
    const MASK: usize;
}



#[cfg(test)]
mod tests {
    use super::*;
}
