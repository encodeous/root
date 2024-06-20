pub mod concepts;
pub mod framework;
pub mod router;
pub mod seqno;

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
    
    fn make_addr<T: Address>(a: &T) -> &[u8; T::BYTES]{
        let m: [u8; T::BYTES];
        m
    }
    
    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);

        let x = bar::<4>().ip.len();
        println!("{}", x);
    }
}
