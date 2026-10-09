//! Every capability's contract, as the core speaks it.
//!
//! One module per capability, each declared once with the crate's `contract!`. What the
//! published documents list is [`all`], so a capability declared here and left out of it
//! is caught by the test that compares the two.

pub mod download;

/// Every capability this build speaks, in the vocabulary's order.
#[must_use]
pub fn all() -> Vec<crate::Capability> {
    vec![
        download::usenet::capability(),
        download::torrent::capability(),
    ]
}

#[cfg(test)]
mod tests;
