//! What a run at boot came to, kept against the restart it ran after, and read back.
//!
//! One record for all of it, beside the configuration: which restart was acted on, what
//! the newest run that reached the stack came to, and why the newest that started nothing
//! did not. Only the newest of each is kept, because the read asks about the latest
//! restart and the newest verification, and a run is told apart from an older one by the
//! restart it names.

use serde::{Deserialize, Serialize};

use crate::config::paths::BOOT as RECORD;
use crate::docker::State;
use crate::error::Problem;
use crate::model::{BootOutcome, BootReport, BootTunnel, BootVerification, LifecycleReport};

use super::super::Ctx;
use super::CHECK;

/// Which restart of this machine has already been acted on, and what the newest runs at
/// one came to.
///
/// The restart is written after the run rather than before it, so a run that was killed
/// part-way is tried again rather than counted as done, and a login that fires twice, a
/// service manager that restarts its agent, and an operator running the command by hand
/// do not each re-verify and re-report the same boot.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Acted {
    /// The moment this machine started, as the run that acted on it read it.
    #[serde(default)]
    pub(super) at: Option<u64>,
    /// What the newest run that reached the stack came to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    verified: Option<BootVerification>,
    /// Why the newest run that started nothing did not, where none has reached the stack
    /// since.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    declined: Option<Declined>,
}

/// Why a run started nothing, against the restart it came after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Declined {
    /// The moment this machine started, where it said.
    #[serde(default)]
    against: Option<u64>,
    /// Why, in the run's words.
    said: String,
}

/// The record as kept, the default where it cannot be read.
pub(super) fn held(ctx: &Ctx) -> Acted {
    super::super::record::beside(ctx, RECORD)
}

/// Write down which restart this run acted on; nothing on a rehearsal.
///
/// A rehearsal that recorded the boot as acted on would make the real run that followed it
/// decline, which is the one way a rehearsal of this could change what it rehearses.
pub(super) async fn acted(ctx: &Ctx) {
    if ctx.dry_run {
        return;
    }
    let at = ctx.started.at().await;
    kept(ctx, |record| record.at = at);
}

/// Keep what a run that reached the stack came to: `report` where the start answered one,
/// the tunnel as it was found, and the sentence raised.
pub(super) async fn verified(
    ctx: &Ctx,
    outcome: BootOutcome,
    report: Option<&LifecycleReport>,
    tunnel: BootTunnel,
    said: Option<&str>,
) {
    if ctx.dry_run {
        return;
    }
    let verification = BootVerification {
        against: ctx.started.at().await,
        at: ctx.seconds(),
        outcome,
        not_back: report.map(not_back).unwrap_or_default(),
        tunnel,
        port: report.and_then(|report| report.forwarded),
        said: said.map(str::to_owned),
    };
    kept(ctx, |record| {
        record.verified = Some(verification);
        record.declined = None;
    });
}

/// Keep why a run started nothing.
pub(super) async fn declined(ctx: &Ctx, said: &str) {
    if ctx.dry_run {
        return;
    }
    let declined = Declined {
        against: ctx.started.at().await,
        said: said.to_owned(),
    };
    kept(ctx, |record| record.declined = Some(declined));
}

/// Change the record as kept and write it back.
fn kept(ctx: &Ctx, change: impl FnOnce(&mut Acted)) {
    let mut record = held(ctx);
    change(&mut record);
    super::super::record::keep_beside(ctx, RECORD, &record);
}

/// Each service the start addressed that is not running.
fn not_back(report: &LifecycleReport) -> Vec<String> {
    report
        .services
        .iter()
        .filter(|service| {
            !matches!(
                service.state,
                State::Running | State::Healthy | State::HostManaged
            )
        })
        .map(|service| service.id.clone())
        .collect()
}

/// What became of the last restart of this machine.
///
/// A verification or a decline is answered as the latest restart's only where it names
/// that restart, so a machine that will not say when it started is answered with none.
///
/// # Errors
///
/// Where the record or the conditions are there and cannot be read, which is never
/// answered as nothing kept.
pub(crate) async fn booted(ctx: &Ctx) -> Result<BootReport, Box<Problem>> {
    let record: Acted =
        super::super::record::read(super::super::targets::beside_env(ctx, RECORD).as_deref())?;
    let conditions = super::super::conditions::read(ctx)?;
    let standing = conditions.get(CHECK);
    let booted_at = ctx.started.at().await;
    let latest = |against: Option<u64>| booted_at.is_some() && against == booted_at;
    Ok(BootReport {
        booted_at,
        verified: record
            .verified
            .clone()
            .filter(|verification| latest(verification.against)),
        last_verified: record.verified,
        declined: record
            .declined
            .filter(|declined| latest(declined.against))
            .map(|declined| declined.said),
        failing_since: standing.and_then(|condition| condition.since.parse().ok()),
        failed_boots: standing.map(|condition| condition.recurrences),
    })
}

#[cfg(test)]
mod tests;
