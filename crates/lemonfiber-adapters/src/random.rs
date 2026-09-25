//! The operating system's randomness.

use lemonfiber_ports::random::Random;

/// The operating system's CSPRNG — the only source trusted to back a secret.
pub struct Os;

impl Random for Os {
    fn bytes(&self, n: usize) -> Option<Vec<u8>> {
        let mut buffer = vec![0u8; n];
        getrandom::fill(&mut buffer).ok().map(|()| buffer)
    }
}

#[cfg(test)]
mod tests;
