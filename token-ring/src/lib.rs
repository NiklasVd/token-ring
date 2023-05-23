pub mod err;
pub mod packet;
pub mod token;
pub mod id;
pub mod serialize;
pub mod signature;
pub mod comm;
pub mod event;
pub mod station;
pub mod pass;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
