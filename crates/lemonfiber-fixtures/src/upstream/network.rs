//! The upstream as an egress guard, and a guard that cannot be reached.

use async_trait::async_trait;
use lemonfiber_ports::service::{Applied, Failure, Tunnelling};

use super::Upstream;

#[async_trait]
impl Tunnelling for Upstream {
    async fn up(&self) -> Result<bool, Failure> {
        self.tell("up".to_owned());
        Ok(true)
    }
    async fn forwarded(&self) -> Result<Option<u16>, Failure> {
        self.tell("forwarded".to_owned());
        Ok(Some(51413))
    }
    async fn secure(&self) -> Result<Applied, Failure> {
        self.tell("secure".to_owned());
        Ok(Applied::OnRestart)
    }
}

/// An egress guard that cannot be reached.
pub struct Unreached;

#[async_trait]
impl Tunnelling for Unreached {
    async fn up(&self) -> Result<bool, Failure> {
        Err(Failure::Unavailable {
            service: "guard".to_owned(),
        })
    }
    async fn forwarded(&self) -> Result<Option<u16>, Failure> {
        Ok(None)
    }
    async fn secure(&self) -> Result<Applied, Failure> {
        Ok(Applied::OnRestart)
    }
}
