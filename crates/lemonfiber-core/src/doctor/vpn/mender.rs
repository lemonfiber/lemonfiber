//! Moving the download client onto the port the provider granted.
//!
//! The one VPN fault whose fix is unambiguous. Nothing is leaking and downloads still
//! arrive; what stops is peers reaching the client, so it cannot seed — the part noticed
//! last, and the reason this is worth putting right rather than only reporting.
//!
//! Starting the stack already applies it, because by then the operator has asked for an
//! action. This is the same correction offered on its own, for an operator who asked what
//! was wrong and then asked for it to be mended.

use std::sync::Arc;

use async_trait::async_trait;

use crate::doctor::{Finding, Mend, Verdict};
use crate::error::Diagnose as _;
use crate::ports::docker::Engine;
use crate::qbittorrent::Qbittorrent;
use crate::repair::{Attempt, Repair};

use super::port_forward::Grant;
use super::probe::{find, read_grant};

/// The finding this answers, as the check names it.
const MISMATCH: &str = "vpn.port-forward-client";

/// Putting the download client back on the forwarded port.
///
/// Holds what the correction needs rather than what the check needs: the gateway to read
/// the grant from, and an authenticated client to move. The client is built where the
/// credentials are — above this — and absent where it could not be authenticated to, which
/// is a client to leave alone rather than one to guess at.
pub(crate) struct PortMender {
    engine: Arc<dyn Engine>,
    project: String,
    gateway: String,
    client: Option<Qbittorrent>,
}

impl PortMender {
    /// A mender for the given pair, with whatever client could be authenticated to.
    pub(crate) fn new(
        engine: Arc<dyn Engine>,
        project: String,
        gateway: String,
        client: Option<Qbittorrent>,
    ) -> Self {
        Self {
            engine,
            project,
            gateway,
            client,
        }
    }

    /// What the provider is granting now.
    ///
    /// Read again rather than taken from the finding that prompted the repair: a grant can
    /// move between looking and acting, and pushing a port the provider has since taken
    /// back is worse than pushing none at all.
    async fn granted(&self) -> Option<u16> {
        let containers = self.engine.list(&self.project).await.ok()?;
        match read_grant(self.engine.as_ref(), find(&containers, &self.gateway)).await {
            Grant::Port(port) => Some(port),
            Grant::Absent | Grant::Unreadable => None,
        }
    }
}

#[async_trait]
impl Mend for PortMender {
    fn repairs(&self, found: &[Finding]) -> Vec<Repair> {
        found
            .iter()
            .filter(|finding| finding.check == MISMATCH)
            .filter(|finding| matches!(finding.verdict, Verdict::Warn(_) | Verdict::Fail(_)))
            .map(|finding| Repair {
                check: finding.check.clone(),
                does: "Move the download client onto the port the provider forwards".to_owned(),
                effects: vec![
                    "The client restarts its listener, so transfers in flight pause briefly"
                        .to_owned(),
                ],
                // The port it was on is in the journal entry, which is what reversing one
                // of these reads — nothing about the client's own state is lost.
                reversible: true,
            })
            .collect()
    }

    async fn mend(&self, _repair: &Repair) -> Attempt {
        let Some(granted) = self.granted().await else {
            return Attempt::Stopped {
                leaving: "the provider is not granting a port now, so the client was left as it was"
                    .to_owned(),
            };
        };
        let Some(client) = &self.client else {
            return Attempt::Stopped {
                leaving: "the download client could not be authenticated to, so it was left as it was"
                    .to_owned(),
            };
        };
        match client.set_listen_port(granted).await {
            Ok(()) => Attempt::Carried,
            Err(failure) => Attempt::Stopped {
                leaving: format!(
                    "the client would not take port {granted}, and stayed where it was — {}",
                    failure.problem().summary
                ),
            },
        }
    }
}
