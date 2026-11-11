//! The operating system's randomness, for the one secret an adapter makes itself: the
//! path its upstream pushes at.

use lemonfiber_ports::random::Random;

/// The operating system's CSPRNG.
pub struct Os;

impl Random for Os {
    fn bytes(&self, n: usize) -> Option<Vec<u8>> {
        let mut buffer = vec![0_u8; n];
        getrandom::fill(&mut buffer).ok().map(|()| buffer)
    }
}

#[cfg(test)]
mod tests;
