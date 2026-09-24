//! Closing the requests nobody ruled on, for as long as an operator leaves this running.
//!
//! **lemonfiber's only clock, and the only thing it does while nobody is watching.** That
//! is the whole difficulty: a reading that quietly declines somebody's request is worse
//! than one that quietly writes a note, and telling them afterwards does not make the
//! decline something anybody agreed to. So four things hold, and each of them is here
//! because without it this would be a policy nobody consented to.
//!
//! **The period is named, never defaulted.** A household that has arranged nothing closes
//! nothing, and a run asked to close things against no arrangement is refused rather than
//! given a figure of this program's choosing.
//!
//! **The arrangement is recorded before anything runs on it**, which is what lets the
//! household reading and the message a member is handed both say what will happen, in
//! advance, on every reading between the arrangement and the first thing it closes.
//! Recording it and running on it are two acts and two runs for that reason.
//!
//! **It stops when the arrangement does.** The record is re-read on every wake, and a run
//! holding a household to a period that has since been withdrawn or replaced is applying
//! one nobody currently agrees to — so it ends and says it ended.
//!
//! **And nothing starts it.** There is no autostart in this product yet, so a clock is a
//! command an operator runs and leaves running; every sentence this writes says so, rather
//! than implying a background this does not have.
//!
//! What each closure comes to is written down as it happens, in the record the refusals
//! next door already keep — so a run stopped at the terminal loses the summary and none of
//! the substance, and the household reading this answers with is where all of it is read.

use std::time::{Duration, SystemTime};

use crate::asking::Expiry;
use crate::error::{Diagnose, Problem};
use crate::household::State;
use crate::model::HouseholdReport;
use crate::ports::service::{Approving as _, HouseholdRequest, Requests as _};

use super::asking::passing_on::{carried, Said};
use super::command::Arranged;
use super::targets::HouseholdAccess;
use super::{arrangement, household, Ctx};

/// How often the clock wakes to look.
///
/// An hour, and it is this command's own rather than the caller's — for a different reason
/// than the guard next door, whose interval a surface could set so as to miss the moment
/// it exists for. Nothing here can be missed: the period is in whole days, so a request
/// that has waited long enough at one wake has still waited long enough at the next. What
/// a surface offered the choice could do instead is spend a household's afternoon asking
/// somebody else's service for nothing, or leave a gap long enough that the arrangement
/// reads as broken. Neither is a choice worth offering.
pub(crate) const SWEEPING: Duration = Duration::from_secs(3_600);

/// Arrange what happens to the requests nobody rules on, or begin doing it.
///
/// Naming a period records it and stops; withdrawing it records that and stops. Naming
/// nothing runs on what was arranged, and holds the terminal until the arrangement changes
/// under it. The two are separate acts because only one of them can ever run unattended,
/// and the household is told about the first long before the second does anything.
///
/// # Errors
///
/// Where a run was asked to begin against an arrangement nobody made, where the period
/// named would close a request before anybody was reminded of it, where there is nowhere
/// to record the arrangement, or where the household itself could not be read afterwards.
pub(crate) async fn expiring(
    ctx: &Ctx,
    arranged: Arranged,
    interval: Duration,
) -> Result<HouseholdReport, Box<Problem>> {
    let agreed = agreeing(ctx, arranged)?;
    let mut said = vec![standing(ctx, &agreed, arranged)];
    if let (Arranged::AsAgreed, Some(after)) = (arranged, agreed.after()) {
        said.extend(closing(ctx, after, interval).await.said(ctx, after));
    }
    let mut report = household::household(ctx, None).await?;
    // What this run did first and the reading under it, the way a decision is answered:
    // an operator who started a clock and came back to it is looking for what it closed,
    // and everything else on the reading is context for that.
    said.append(&mut report.findings);
    report.findings = said;
    Ok(report)
}

/// The arrangement this run is to work from, written down where the run makes one.
///
/// A rehearsal writes nothing and still answers with what it would have arranged, because
/// what an operator rehearsing this wants to know is which requests a period would reach —
/// and a run that recorded the period on the way to telling them would have arranged it.
fn agreeing(ctx: &Ctx, arranged: Arranged) -> Result<Expiry, Box<Problem>> {
    match arranged {
        Arranged::After(days) => {
            if Expiry::too_soon(days) {
                return Err(Box::new(crate::asking::sooner_than_the_reminder(days)));
            }
            let agreed = Expiry::agreed_to(days, crate::instant::written(ctx.clock.now()));
            if !ctx.dry_run {
                arrangement::keep(ctx, &agreed)?;
            }
            Ok(agreed)
        }
        Arranged::Never => {
            let withdrawn = Expiry::default();
            if !ctx.dry_run {
                arrangement::keep(ctx, &withdrawn)?;
            }
            Ok(withdrawn)
        }
        // Refused rather than defaulted, and this is the whole of what keeps an expiry
        // from being something that happens to a household rather than something it
        // arranged.
        Arranged::AsAgreed => {
            let agreed = arrangement::load(ctx);
            if agreed.after().is_none() {
                return Err(Box::new(crate::asking::nothing_agreed()));
            }
            Ok(agreed)
        }
    }
}

/// What the household now stands under, as the operator reads it back.
///
/// **It says what runs it, because nothing does.** An arrangement recorded here closes
/// nothing by itself, and a sentence that left that out would describe a background this
/// product has not got — which is the same untruth as a request closed against a period
/// nobody named, arriving from the other direction.
fn standing(ctx: &Ctx, agreed: &Expiry, arranged: Arranged) -> String {
    let Some(after) = agreed.after() else {
        return "nothing is closed for waiting here: every request waits until somebody \
                rules on it"
            .to_owned();
    };
    if matches!(arranged, Arranged::AsAgreed) {
        return format!(
            "closing what nobody has ruled on after {after} days, and doing it for as long \
             as this is left running"
        );
    }
    let rehearsed = if ctx.dry_run {
        " — rehearsed, and nothing was arranged"
    } else {
        ""
    };
    format!(
        "a request may now wait {after} days before it is closed, and the household is \
         told so before anything reaches it — nothing closes one until you run \
         `lemonfiber household expiring`, because this product starts nothing by \
         itself{rehearsed}"
    )
}

/// What a run of the clock came to.
///
/// Counted rather than listed, and deliberately: a clock left running for a month would
/// otherwise carry a line for every hour a service was down, and the summary an operator
/// reads at the end of it would be the one thing they could not read. What each closure
/// came to is in the record beside its reason, which is what the reading under this shows.
#[derive(Default)]
struct CameTo {
    /// How many requests it closed.
    closed: usize,
    /// How many wakes could not look at all.
    missed: usize,
    /// What stopped the last of those, in the operator's own words.
    why: Option<String>,
}

impl CameTo {
    /// Write down that one wake came to nothing, and why.
    ///
    /// The latest reason rather than the first: a service that was down and is now
    /// answering differently is a different thing to look at, and the one an operator
    /// reading this at the end can still act on is the last.
    fn missed(&mut self, why: &str) {
        self.missed += 1;
        self.why = Some(why.to_owned());
    }

    /// What the run comes to, as the lines an operator reads.
    fn said(&self, ctx: &Ctx, after: u32) -> Vec<String> {
        let mut said = vec![match self.closed {
            0 => format!("nothing had waited {after} days, so nothing was closed"),
            closed => format!(
                "{} {closed} request{} nobody had ruled on, for having waited {after} days — \
                 what each of them was told is beside it below",
                if ctx.dry_run { "would close" } else { "closed" },
                crate::plural::s(closed),
            ),
        }];
        if let Some(why) = &self.why {
            said.push(format!(
                "{} look{} came to nothing, the last of them because {why}",
                self.missed,
                crate::plural::s(self.missed)
            ));
        }
        said
    }
}

/// Close what has waited too long, again and again, until the arrangement changes.
///
/// **What ends it is the record rather than a signal.** A run holding a household to a
/// period the operator has since withdrawn or replaced is applying one nobody currently
/// agrees to, so the arrangement is re-read on every wake and any disagreement ends the
/// run. It is also how a second run replaces a first: the write that arranges the new
/// period is the one that stops the old one.
///
/// A rehearsal looks once and does not wait, because what it is asked is which requests a
/// period reaches and not what will happen in an hour.
async fn closing(ctx: &Ctx, after: u32, interval: Duration) -> CameTo {
    let mut came_to = CameTo::default();
    let mut arranged = true;
    while arranged {
        swept(ctx, after, &mut came_to).await;
        if ctx.dry_run {
            return came_to;
        }
        tokio::time::sleep(interval).await;
        // Read again rather than remembered, which is the whole of what makes stopping a
        // matter of the record: an operator who withdrew the arrangement an hour ago has
        // withdrawn it from this run too, and one who replaced the period has replaced it
        // here rather than started a second run beside this one.
        arranged = arrangement::load(ctx).after() == Some(after);
    }
    came_to
}

/// One look at the household, closing everything that has waited longer than it agreed to.
///
/// Nothing here is a failure to report: a service that will not answer this hour is one to
/// ask again next hour, and ending a month-long run over a restart would be a clock that
/// stopped the first time anything moved.
async fn swept(ctx: &Ctx, after: u32, came_to: &mut CameTo) {
    let manifest = match ctx.stack.checked_manifest(ctx.today()) {
        Ok(manifest) => manifest,
        Err(err) => return came_to.missed(&err.problem().summary),
    };
    let access = match household::reaching(ctx, &manifest.services).await {
        Ok(access) => access,
        Err(reason) => return came_to.missed(&reason),
    };
    let Ok(asked) = access.seerr.requests().await else {
        return came_to.missed(
            "the request service's own record could not be read, so nothing was looked at",
        );
    };
    for request in overdue(&asked, after, ctx.clock.now()) {
        close(ctx, &access, request, after, &asked, came_to).await;
    }
}

/// The requests nobody has ruled on that have waited longer than the household agreed to.
///
/// **Counted the way the reminder counts, from the same reading.** A request the operator
/// was told had waited nine days and one closed for having waited nine days have to be the
/// same request, or the reminder is about a different arithmetic from the thing it warns
/// about.
///
/// A request whose date this cannot read is never closed. It has not been shown to have
/// waited at all, and closing one on no evidence is the opposite of what a period is for.
fn overdue(asked: &[HouseholdRequest], after: u32, now: SystemTime) -> Vec<&HouseholdRequest> {
    asked
        .iter()
        .filter(|held| {
            State::of(held.request_status, held.media_status) == Some(State::WaitingForApproval)
        })
        .filter(|held| {
            crate::asking::waiting_for(held.made.as_deref(), now)
                .is_some_and(|days| days >= u64::from(after))
        })
        .collect()
}

/// Close one request, and carry the reason to whoever asked for it.
///
/// The decision first and the words second, in that order and for the same reason the
/// operator's own refusal has them in that order: a reason recorded for a closure that
/// never happened would be shown to somebody beside a request that is still waiting.
async fn close(
    ctx: &Ctx,
    access: &HouseholdAccess,
    request: &HouseholdRequest,
    after: u32,
    asked: &[HouseholdRequest],
    came_to: &mut CameTo,
) {
    if ctx.dry_run {
        came_to.closed += 1;
        return;
    }
    if access.seerr.decide(request.id, false).await.is_err() {
        return came_to.missed(&format!(
            "the request service would not close request {}, so it is still waiting on you",
            request.id
        ));
    }
    // What became of the words is not carried up into the summary: it is written beside
    // the reason as it happens, and the household this answers with is where it is read.
    // A run of a month would otherwise end with a line per closure and no summary in it.
    let reason = why(after);
    let _carried = carried(ctx, access, request.id, Some(Said::RanOut(&reason)), asked).await;
    came_to.closed += 1;
}

/// What the person who asked is told, which is the whole of what they are told.
///
/// **The decline itself is not in it.** The request service sends that the moment the
/// decision goes through, and a second message saying so is the duplicate this product
/// refuses to send — so what travels is why, and why is that nobody answered.
///
/// It says to ask again, because that is the difference between this and a refusal. An
/// operator who turned something down has decided something; a request that ran out was
/// never decided at all, and somebody who read the two the same way would take a silence
/// for a no.
fn why(after: u32) -> String {
    format!(
        "nobody ruled on it within {after} days, so it was closed; ask again if you still \
         want it"
    )
}

#[cfg(test)]
mod tests;
