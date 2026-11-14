//! A news server that answers every conversation from a script.

use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_ports::nntp::{Endpoint, Nntp, Unreachable};

/// A news server replying to every conversation with the same lines, or never reached.
pub struct Dialled(Result<Vec<String>, Unreachable>);

impl Dialled {
    /// A server replying with these lines: the greeting, then one reply per command.
    #[must_use]
    pub fn replying(lines: &[&str]) -> Arc<Self> {
        Arc::new(Self(Ok(lines
            .iter()
            .map(|line| (*line).to_owned())
            .collect())))
    }

    /// A server at `host` that could not be reached, for this reason.
    #[must_use]
    pub fn unreached(host: &str, reason: &str) -> Arc<Self> {
        Arc::new(Self(Err(Unreachable {
            host: host.to_owned(),
            reason: reason.to_owned(),
        })))
    }
}

#[async_trait]
impl Nntp for Dialled {
    async fn converse(
        &self,
        _endpoint: &Endpoint,
        _commands: &[String],
    ) -> Result<Vec<String>, Unreachable> {
        self.0.clone()
    }
}
