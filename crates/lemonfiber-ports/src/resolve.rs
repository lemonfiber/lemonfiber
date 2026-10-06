//! Asking which addresses a name stands for.
//!
//! Behind a port because the answer is the world's and differs on every machine and
//! every hour: a test that asked the real resolver would pass where it was written, and
//! a decision taken over an address has to be tested against the addresses that decide
//! it, which only a stand-in can be made to give.
//!
//! See `.docs/architecture/ports-and-adapters.md`.

use std::net::IpAddr;

use async_trait::async_trait;

/// Asks which addresses a name stands for.
#[async_trait]
pub trait Resolver: Send + Sync {
    /// Every address `host` stands for when it is reached on `port`, or why none could
    /// be told.
    ///
    /// # Errors
    ///
    /// Where the name stands for nothing, or the resolver could not be asked, in its
    /// own words.
    async fn addresses(&self, host: &str, port: u16) -> Result<Vec<IpAddr>, String>;
}
