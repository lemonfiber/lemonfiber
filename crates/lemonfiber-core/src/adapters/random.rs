//! The operating system's randomness.

use crate::ports::random::Random;

/// The operating system's CSPRNG — the only source trusted to back a secret.
pub struct Os;

impl Random for Os {
    fn bytes(&self, n: usize) -> Option<Vec<u8>> {
        let mut buffer = vec![0u8; n];
        getrandom::getrandom(&mut buffer).ok().map(|()| buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::Os;
    use crate::ports::random::Random;

    #[test]
    fn it_returns_the_requested_number_of_bytes() {
        let bytes = Os.bytes(24);
        assert_eq!(bytes.map(|bytes| bytes.len()), Some(24));
    }

    #[test]
    fn two_draws_do_not_come_out_the_same() {
        // Not a statistical test — just that the source is not a fixed constant.
        // Two 24-byte draws colliding is far past astronomically unlikely.
        assert_ne!(Os.bytes(24), Os.bytes(24));
    }
}
