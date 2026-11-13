//! The upstream as a part claimed first.

use async_trait::async_trait;
use lemonfiber_ports::service::{Claim, Failure, Owner, Ownership};

use super::Upstream;

#[async_trait]
impl Ownership for Upstream {
    async fn owner(&self) -> Result<Owner, Failure> {
        self.tell("owner".to_owned());
        Ok(Owner::Open)
    }
    async fn claim(&self) -> Result<Claim, Failure> {
        self.tell("claim".to_owned());
        Ok(Claim::ON_RESTART)
    }
    async fn claimed_by(&self) -> Result<Option<String>, Failure> {
        self.tell("claimed-by".to_owned());
        Ok(Some("4815162342".to_owned()))
    }
}
