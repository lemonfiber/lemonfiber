//! Keeping the download client on the port the provider actually granted.
//!
//! A tunnel that drops and comes back is commonly granted a different port. The
//! gateway records the new one, and the download client goes on listening on
//! yesterday's — at which point everything looks correct from inside and nobody
//! outside can reach it. Torrents still download, so nothing announces it; only
//! the seeding stops, which is the part an operator notices last.
//!
//! So the grant is compared with what the client is listening on, and a mismatch
//! is pushed rather than merely reported: this is the one VPN fault where the fix
//! is unambiguous, and where leaving it to be read about means it stays broken
//! until somebody happens to look.
//!
//! Recorded either way. A port that changed and was re-pushed is a thing that
//! happened to the stack, and an operator reading back why seeding stopped for an
//! hour needs to find it.

use std::path::Path;

use crate::doctor::vpn::Forwarding;
use crate::error::Diagnose;
use crate::journal::{Change, Kind};

use super::targets::{download_targets, torrent_client};
use super::Ctx;

/// The setting a re-pushed port is journalled under, so a change to it reads like
/// any other change lemonfiber made.
pub const SETTING: &str = "qbittorrent.listen_port";

/// What one pass over the forwarded port did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pushed {
    /// The client was already on the granted port, or there was nothing to push.
    Unchanged,
    /// The client was moved to the granted port.
    Moved {
        /// What it was listening on before, where that was known.
        from: Option<u16>,
        /// What it is listening on now.
        to: u16,
    },
    /// A push was needed and could not be made.
    Refused {
        /// What the client said, in its own words.
        reason: String,
    },
}

impl Pushed {
    /// The change this amounts to, for the journal — nothing where nothing moved.
    #[must_use]
    pub fn change(&self, stamp: &str) -> Option<Change> {
        match self {
            Self::Moved { from, to } => Some(Change {
                at: stamp.to_owned(),
                operation: "vpn port forwarding".to_owned(),
                target: "qbittorrent".to_owned(),
                kind: Kind::Set {
                    key: SETTING.to_owned(),
                    previous: from.map(|port| port.to_string()),
                    current: to.to_string(),
                },
            }),
            Self::Unchanged | Self::Refused { .. } => None,
        }
    }

    /// The line an operator reads.
    #[must_use]
    pub fn said(&self) -> Option<String> {
        match self {
            Self::Unchanged => None,
            Self::Moved { from, to } => Some(match from {
                Some(before) => format!(
                    "the forwarded port changed from {before} to {to}; the download client was \
                     moved to it"
                ),
                None => format!("the download client was set to listen on the forwarded port {to}"),
            }),
            Self::Refused { reason } => Some(format!(
                "the forwarded port could not be pushed to the download client: {reason}"
            )),
        }
    }
}

/// Move the client onto the granted port where it is not already there.
///
/// `set` is the write itself, kept as a parameter so the decision is testable
/// without a client: everything above this line is about *whether* to write, and
/// this function is only about which write and what to record.
pub async fn push<F, E>(forwarding: Forwarding, set: F) -> Pushed
where
    F: FnOnce(u16) -> E,
    E: std::future::Future<Output = Result<(), String>>,
{
    let Some(port) = forwarding.to_push() else {
        return Pushed::Unchanged;
    };
    match set(port).await {
        Ok(()) => Pushed::Moved {
            from: forwarding.listening,
            to: port,
        },
        Err(reason) => Pushed::Refused { reason },
    }
}

/// Read the granted port and the client's own, and move the client where they
/// differ.
///
/// The one VPN fault whose fix is unambiguous, so it is applied rather than
/// reported. A client that cannot be authenticated to is left alone: it can be
/// neither read nor corrected, and guessing would be worse than saying nothing.
pub async fn reconcile(ctx: &Ctx, granted: Option<u16>, project: Option<&Path>) -> Pushed {
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        return Pushed::Unchanged;
    };
    let targets = download_targets(&manifest.services, project);
    let Some(client) = torrent_client(ctx, &targets) else {
        return Pushed::Unchanged;
    };
    let forwarding = Forwarding {
        granted,
        listening: client.listen_port().await.ok(),
    };
    push(forwarding, |port| async move {
        client
            .set_listen_port(port)
            .await
            .map_err(|failure| failure.problem().summary)
    })
    .await
}

/// What the torrent client says it is listening on, where there is one and it can
/// be authenticated to.
pub(crate) async fn listening_port(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    project: Option<&std::path::Path>,
) -> Option<u16> {
    let targets = download_targets(&manifest.services, project);
    torrent_client(ctx, &targets)?.listen_port().await.ok()
}

/// What starting the stack does about the forwarded port.
///
/// The tunnel has just come up, which is exactly when the provider grants a port
/// — commonly a different one from last time. Applied here rather than offered
/// because the operator has already asked for an action, and a client left on
/// yesterday's port looks entirely healthy from inside while nobody outside can
/// reach it. A diagnosis, which is only looking, offers the same fix instead of
/// making it.
pub(crate) async fn after_start(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
) -> Option<String> {
    let granted = crate::doctor::vpn::granted_port(
        ctx.engine.as_ref(),
        &ctx.settings.project,
        manifest,
        ctx.settings.port_forward.enabled,
    )
    .await;
    let project = super::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    reconcile(ctx, granted, project.as_deref()).await.said()
}

#[cfg(test)]
mod tests;
