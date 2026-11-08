//! The outside watcher, held to the monitors the stack owes it: one for every critical
//! service, every household entry point and every hosted job, each under lemonfiber's own
//! tag, and none for what is gone.

use std::collections::BTreeSet;

use crate::app::watcher::covering::{wanted, Wanted};
use crate::app::watcher::kept::{self, Kept, Made};
use crate::app::Ctx;
use crate::plugin::Installed;
use crate::ports::service::{Covers, Failure, Monitor, Monitoring};
use crate::seed::{State, Wiring};
use crate::wiring::Fillers;

/// What the report calls this connection.
pub(super) const CONNECTION: &str = "The outside watcher's monitors";

/// Hold the watcher to the monitors the stack's `services`, the `installed` plugins' and
/// this machine's hosted jobs owe it: nothing where nothing fills `status.watch`.
pub(super) async fn seed_watcher(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    installed: &[Installed],
    fillers: &Fillers,
) -> Option<Wiring> {
    let watcher = crate::app::watcher::asked(ctx, fillers).await?;
    let should = wanted(ctx, services, installed, fillers).await;
    let held = kept::held(ctx);
    let monitors = match watcher.monitors().await {
        Ok(monitors) => monitors,
        Err(failure) => return Some(settled(crate::seed::unreached(&failure))),
    };
    let gone = gone(&held, &monitors, &should);
    if ctx.dry_run {
        let same = gone.is_empty() && should.iter().all(|one| standing(&held, &monitors, one));
        return Some(settled(if same {
            State::AlreadyWired
        } else {
            State::WouldWire {
                yours: Some(said(held.made.len())),
                ours: Some(said(should.len())),
            }
        }));
    }
    let (record, failed, changed) =
        provisioned(watcher.as_ref(), &held, &monitors, &should, gone).await;
    kept::keep(ctx, &record);
    Some(settled(match failed {
        Some(failure) => crate::seed::unreached(&failure),
        None if changed => State::Wired,
        None => State::AlreadyWired,
    }))
}

/// Provision every monitor `should` names and retire every tag `gone` names, answering
/// the record that leaves, the first failure, and whether anything changed.
///
/// A monitor that could not be provisioned or retired keeps what the record held of it,
/// so the pairing it had is not lost to one refusal.
async fn provisioned(
    watcher: &dyn Monitoring,
    held: &Kept,
    monitors: &[Monitor],
    should: &[Wanted],
    gone: BTreeSet<String>,
) -> (Kept, Option<Failure>, bool) {
    let mut record = Kept::default();
    let mut failed = None;
    let mut changed = false;
    for one in should {
        let tag = one.covers.tag();
        match watcher.provision(&one.provision).await {
            Ok(answered) => {
                let made = Made::provisioned(one, answered);
                changed |= !standing(held, monitors, one) || held.made.get(&tag) != Some(&made);
                record.made.insert(tag, made);
            }
            Err(failure) => {
                if let Some(made) = held.made.get(&tag) {
                    record.made.insert(tag, made.clone());
                }
                failed.get_or_insert(failure);
            }
        }
    }
    for tag in gone {
        match watcher.retire(&tag).await {
            Ok(()) => changed = true,
            Err(failure) => {
                if let Some(made) = held.made.get(&tag) {
                    record.made.insert(tag, made.clone());
                }
                failed.get_or_insert(failure);
            }
        }
    }
    (record, failed, changed)
}

/// Every tag lemonfiber gave a monitor, by its record or as the watcher holds it, that
/// covers nothing the stack still owes a monitor.
fn gone(held: &Kept, monitors: &[Monitor], should: &[Wanted]) -> BTreeSet<String> {
    let owed: BTreeSet<String> = should.iter().map(|one| one.covers.tag()).collect();
    held.made
        .keys()
        .cloned()
        .chain(
            monitors
                .iter()
                .filter_map(|monitor| monitor.tag.clone())
                .filter(|tag| Covers::read(tag).is_some()),
        )
        .filter(|tag| !owed.contains(tag))
        .collect()
}

/// Whether the watcher already holds `one` as it was last provisioned.
fn standing(held: &Kept, monitors: &[Monitor], one: &Wanted) -> bool {
    held.made.get(&one.covers.tag()).is_some_and(|made| {
        made.asked == one.provision && monitors.iter().any(|monitor| monitor.id == made.id)
    })
}

/// Monitors, by how many.
fn said(count: usize) -> String {
    format!("{count} monitor{}", crate::plural::s(count))
}

/// Provision the watcher again, as a change to what it is made from owes it, as a seed
/// pass does it, and say how that came out: nothing where the stack or the record of what
/// is installed cannot be read, or nothing fills `status.watch`.
pub(crate) async fn remonitor(ctx: &Ctx) -> Option<State> {
    let (services, installed, fillers) = super::managed(ctx)?;
    seed_watcher(ctx, &services, &installed, &fillers)
        .await
        .map(|wiring| wiring.state)
}

/// [`remonitor`], as a start says it: where the watcher was given monitors, or could not be,
/// and nothing where it already held them or nothing fills `status.watch`.
pub(crate) async fn remonitored(ctx: &Ctx) -> Option<String> {
    remonitor(ctx).await.as_ref().and_then(sentence)
}

/// What a pass resting in `state` is said as, where it is worth saying.
fn sentence(state: &State) -> Option<String> {
    match state {
        State::Wired => Some(PROVISIONED.to_owned()),
        State::Failed { detail: why }
        | State::Skipped { reason: why }
        | State::Unmatched { reason: why } => Some(format!("{UNPROVISIONED}: {why}")),
        _ => None,
    }
}

/// What a start says where the watcher was given its monitors.
const PROVISIONED: &str = "The outside watcher holds a monitor for every critical service, \
                           household entry point and hosted job, and none for what is gone";

/// What a start says where the watcher could not be given them.
const UNPROVISIONED: &str = "The outside watcher's monitors could not all be provisioned";

/// This connection, resting in `state`.
fn settled(state: State) -> Wiring {
    Wiring::settled(CONNECTION.to_owned(), state)
}
