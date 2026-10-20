//! Passing the egress guard's forwarded port to every torrent client inside it.
//!
//! The guard's filler is asked over `network.egress-guard`, and each client is set over
//! `download.torrent`, so the port crosses through the core and nothing is mounted into
//! the guard or opened for it to call.

mod deciding;
mod reaching;

use std::path::Path;
use std::time::Duration;

use lemonfiber_contract::capabilities::download::torrent;
use lemonfiber_manifest::Manifest;

use crate::condition::Conditions;
use crate::config::store;
use crate::doctor::{Category, Finding, Verdict};
use crate::error::Diagnose as _;

use super::targets::{host_fillers, project_directory};
use super::Ctx;

pub use deciding::{acts, passed, Asking, CameTo, Client, Last, Pass};

/// How often the guard is asked while the stack runs.
pub const EVERY: Duration = Duration::from_secs(60);

/// The operation a passed port is journalled under.
pub const OPERATION: &str = "vpn port forwarding";

/// The resource a passed port is journalled as.
pub const RESOURCE: &str = "peer port";

/// The field of that resource a passed port sets.
pub const FIELD: &str = "peer_port";

/// The check a port that was not passed is raised under.
pub const CHECK: &str = "vpn.port-passed";

/// The record of the guard's last answer every client took.
const RECORD: &str = "forwarded.json";

/// Ask the guard for its forwarded port and pass it to every torrent client inside it
/// where `asking` says to, journalling each port set as it is set.
///
/// Nothing where this is a rehearsal, no guard fills the stack's egress, or the guard
/// cannot tell.
pub async fn pass(
    ctx: &Ctx,
    manifest: &Manifest,
    project: Option<&Path>,
    asking: Asking,
) -> Option<Pass> {
    if ctx.dry_run {
        return None;
    }
    let fillers = host_fillers(ctx, manifest, project);
    let answer = reaching::guard(ctx, &fillers)
        .await?
        .forwarded()
        .await
        .ok()?;
    let mut pass = Pass {
        answer,
        ..Pass::default()
    };
    if !acts(asking, answer, super::record::beside(ctx, RECORD)) {
        return Some(pass);
    }
    for inside in reaching::inside(ctx, &fillers).await {
        let client = Client {
            came_to: to_client(answer, inside.client.as_deref()).await,
            service: inside.service,
        };
        if let Some(why) = journalled(ctx, client.change(answer, &ctx.stamp())) {
            pass.unrecorded = Some(why);
        }
        pass.clients.push(client);
    }
    if pass.passed() {
        super::record::keep_beside(ctx, RECORD, &Last { port: answer });
    }
    Some(pass)
}

/// One pass on the minute, against the stack as it stands: what the watch and the
/// dashboard's gather each run every [`EVERY`].
pub async fn on_the_minute(ctx: &Ctx) -> Option<Pass> {
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok()?;
    on_disk(ctx, &manifest, Asking::Changed).await
}

/// One pass against the stack as it is written to disk.
async fn on_disk(ctx: &Ctx, manifest: &Manifest, asking: Asking) -> Option<Pass> {
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    pass(ctx, manifest, project.as_deref(), asking).await
}

/// Raise the port not passed in `conditions`, or clear it, as `pass` found it.
pub fn observe(conditions: &mut Conditions, pass: &Pass, stamp: &str) {
    conditions.observe(CHECK, pass.fault().as_ref(), stamp);
}

/// What passing `answer` to one client came to.
async fn to_client(answer: Option<u16>, client: Option<&dyn torrent::Fills>) -> CameTo {
    let Some(client) = client else {
        return CameTo::Refused {
            reason: "it could not be reached".to_owned(),
        };
    };
    let holds = client
        .peer_port()
        .await
        .map_err(|failure| failure.problem().summary);
    passed(answer, holds, |port| async move {
        client
            .set_peer_port(port)
            .await
            .map_err(|failure| failure.problem().summary)
    })
    .await
}

/// Journal `change` where there is one, answering why it could not be where it could not.
fn journalled(ctx: &Ctx, change: Option<crate::journal::Change>) -> Option<String> {
    let change = change?;
    let written = super::targets::layout(ctx).map_or(Err(store::Failure::Nowhere), |paths| {
        super::recover::journalled(
            &paths.journal(),
            std::slice::from_ref(&change),
            ctx.seams.random.as_ref(),
        )
    });
    written
        .err()
        .map(|failure| super::recover::unrecorded("Passing the forwarded port", &failure).summary)
}

/// Raise or clear the port not passed in the conditions kept, as `pass` found it.
fn raised(ctx: &Ctx, pass: &Pass) {
    let mut conditions = super::conditions::load(ctx);
    observe(&mut conditions, pass, &ctx.stamp());
    super::conditions::save(ctx, &conditions);
}

/// Say what `pass` came to, and raise or clear the port not passed.
async fn told(ctx: &Ctx, pass: &Pass) -> Vec<String> {
    raised(ctx, pass);
    let said = pass.said();
    for line in &said {
        ctx.narrator.say(line).await;
    }
    said
}

/// The port the first torrent client inside the guard takes peers on, where it answers.
pub(crate) async fn listening_port(
    ctx: &Ctx,
    manifest: &Manifest,
    project: Option<&Path>,
) -> Option<u16> {
    let fillers = host_fillers(ctx, manifest, project);
    let inside = reaching::inside(ctx, &fillers).await;
    let client = inside.iter().find_map(|one| one.client.as_deref())?;
    client.peer_port().await.ok().flatten()
}

/// What starting the stack does about the forwarded port: one pass afresh, raised where a
/// client did not take it.
pub(crate) async fn after_start(ctx: &Ctx, manifest: &Manifest) -> Option<String> {
    let pass = on_disk(ctx, manifest, Asking::Afresh).await?;
    raised(ctx, &pass);
    let said = pass.said();
    (!said.is_empty()).then(|| said.join("; "))
}

/// What a diagnosis does about the forwarded port once its VPN check ran against a
/// running stack: one pass afresh, said and raised where a client did not take it.
pub(crate) async fn after_diagnosis(
    ctx: &Ctx,
    manifest: &Manifest,
    findings: &[Finding],
) -> Vec<String> {
    if !ran_against_a_running_stack(findings) {
        return Vec::new();
    }
    match on_disk(ctx, manifest, Asking::Afresh).await {
        Some(pass) => told(ctx, &pass).await,
        None => Vec::new(),
    }
}

/// Whether the VPN check found anything to say about a running stack.
fn ran_against_a_running_stack(findings: &[Finding]) -> bool {
    findings.iter().any(|finding| {
        finding.category == Category::Vpn && !matches!(finding.verdict, Verdict::Skipped { .. })
    })
}

/// The watch's own pass on the minute, said and raised.
pub(crate) async fn watched(ctx: &Ctx) {
    if let Some(pass) = on_the_minute(ctx).await {
        told(ctx, &pass).await;
    }
}

#[cfg(test)]
mod tests;
