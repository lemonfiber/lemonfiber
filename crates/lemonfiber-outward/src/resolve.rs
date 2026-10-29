//! The operating system's resolver.

use std::net::IpAddr;

use async_trait::async_trait;
use lemonfiber_ports::resolve::Resolver;

/// The resolver this machine is configured with, asked as anything else on it would ask.
pub struct Lookup;

#[async_trait]
impl Resolver for Lookup {
    async fn addresses(&self, host: &str, port: u16) -> Result<Vec<IpAddr>, String> {
        tokio::net::lookup_host((host, port))
            .await
            .map(|found| found.map(|address| address.ip()).collect())
            .map_err(|why| why.to_string())
    }
}

#[cfg(test)]
mod tests;
