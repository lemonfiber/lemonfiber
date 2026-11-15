//! The upstream as a stream guard.

use async_trait::async_trait;
use lemonfiber_ports::service::{Failure, Guarded, Guarding, Presented};

use super::media::declared;
use super::Upstream;

/// The envelope a guard is applied, around the media server's declaration.
#[must_use]
pub fn guarded() -> Guarded {
    Guarded {
        declared: declared(),
        upstream: "http://media:8096".to_owned(),
        adapter: Some("http://media-adapter:8080".to_owned()),
        proxy: "10.80.96.3".to_owned(),
        pass: "http://10.80.96.1:9000/pass".to_owned(),
    }
}

#[async_trait]
impl Guarding for Upstream {
    async fn guard(&self, guarded: &Guarded) -> Result<(), Failure> {
        self.tell(format!("guard {guarded:?}"));
        Ok(())
    }
    async fn guarding(&self) -> Result<Option<Guarded>, Failure> {
        self.tell("guarding".to_owned());
        Ok(Some(guarded()))
    }
    async fn certificate(&self) -> Result<Presented, Failure> {
        self.tell("certificate".to_owned());
        Ok(Presented {
            fingerprint: "ab".repeat(32),
        })
    }
}
