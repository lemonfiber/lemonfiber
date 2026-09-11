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

use super::findings::PORT_MISMATCH_CHECK;
use super::forwarding::Forwarding;
use super::port_forward::grant_at;

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
}

#[async_trait]
impl Mend for PortMender {
    fn repairs(&self, found: &[Finding]) -> Vec<Repair> {
        found
            .iter()
            .filter(|finding| finding.check == PORT_MISMATCH_CHECK)
            .filter(|finding| matches!(finding.verdict, Verdict::Warn(_) | Verdict::Fail(_)))
            .map(|finding| Repair {
                check: finding.check.clone(),
                does: "Move the download client onto the port the provider forwards".to_owned(),
                effects: vec![
                    "The client restarts its listener, so transfers in flight pause briefly"
                        .to_owned(),
                ],
                // Not reversible, and said so rather than assumed. Nothing on the repair
                // path writes a journal entry, so there is nothing an undo could read; a
                // repair that claimed otherwise would be promising the operator a way back
                // that does not exist. The client's own port is the only thing changed, and
                // starting the stack sets it again from the grant.
                reversible: false,
            })
            .collect()
    }

    async fn mend(&self, _repair: &Repair) -> Attempt {
        let Some(client) = &self.client else {
            return Attempt::Stopped {
                leaving:
                    "the download client could not be authenticated to, so it was left as it was"
                        .to_owned(),
            };
        };
        // Read again rather than trusting the number the diagnosis saw: a grant can move
        // between looking and acting, and pushing a port the provider has since taken back
        // is worse than pushing none at all. The client is asked afresh for the same reason.
        let forwarding = Forwarding {
            granted: grant_at(self.engine.as_ref(), &self.project, &self.gateway).await,
            listening: client.listen_port().await.ok(),
        };
        let Some(granted) = forwarding.to_push() else {
            // Nothing to move it to, or it is already there. A write that changes nothing is
            // still a write, and this one restarts the client's listener.
            return Attempt::Stopped {
                leaving: "the client was left where it was, having nowhere else to be".to_owned(),
            };
        };
        match client.set_listen_port(granted).await {
            Ok(()) => Attempt::carried(),
            Err(failure) => Attempt::Stopped {
                leaving: format!(
                    "the client would not take port {granted}, and stayed where it was — {}",
                    failure.problem().summary
                ),
            },
        }
    }
}
