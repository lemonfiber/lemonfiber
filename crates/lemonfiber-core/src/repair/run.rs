//! Carrying out the repairs a diagnosis offered, and proving they worked.
//!
//! The order is the whole of the design: look, offer, confirm, act, then look again — and
//! only the second look decides what is reported. A repair whose command returned an exit
//! code of zero has proved nothing; what proves it is the check that raised the finding
//! failing to raise it again.
//!
//! Nothing here decides what may be offered. That is [`crate::repair`]'s, purely: what has
//! been declined, what has been tried too often and which findings share a cause are all
//! settled before this module sees them. What is here is the running of it.
//!
//! How much of it a run was given consent for is [`consent`]'s, which is the half a
//! surface with no terminal to hold a question open in has to send with the request.

mod attempts;
mod consent;
mod proving;
mod remembering;
mod telling;

pub use consent::{Consent, STALE};
use proving::{carried, looked};
use remembering::{beyond, declined, recorded, remembered};
pub(crate) use telling::told;

use crate::config::paths::Paths;
use crate::doctor::{Check, Finding};
use crate::error::{Diagnose as _, Problem, Remedy, Severity, State};
use crate::journal::Undo;
use crate::repair::{self, Outcome, Repair, Stance, Writing};

use crate::app::Ctx;

/// Whoever decides, for this run, whether a repair goes ahead.
///
/// A trait rather than a closure: a surface that builds one to capture what it asks with
/// builds a function of its own, and the sequence here would be copied around it. What
/// asks is the surface's business either way — this only needs the answer.
///
/// Shareable across threads, because a surface that answers a request with a name for
/// the work hands the run to a runtime and the sequence carries this into it.
pub trait Confirm: Sync {
    /// Whether this repair may be carried out.
    fn agreed(&self, repair: &Repair) -> bool;

    /// Whether the offer this consent was given for is the offer that stands now.
    ///
    /// Asked once, before anything is carried out. A terminal answers yes by
    /// construction: the question and the answer are the same run over the same
    /// look. A surface whose consent crossed a request boundary read an offer that
    /// this run has just looked again for, and an answer to the old one is an
    /// answer to a question nobody is asking any more.
    fn stands(&self, _offered: &[Repair]) -> bool {
        true
    }
}

/// One repair, and what became of it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Mended {
    /// What was proposed.
    pub repair: Repair,
    /// How it turned out, once the check was asked again.
    pub outcome: Outcome,
}

/// A repair that has run out of chances, and where to go instead.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Beyond {
    /// The check whose fault has outlasted every attempt at it.
    pub check: String,
    /// What to do about it now that lemonfiber has stopped offering to.
    pub remedy: Remedy,
}

/// What a repairing run offered, and what it did.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, schemars::JsonSchema)]
#[schemars(rename = "RepairReport")]
pub struct Report {
    /// What could be put right, whether or not it was.
    pub offered: Vec<Repair>,
    /// What this offer is, so consent given for it can name which offer it read.
    ///
    /// Carried on every report rather than only on the ones that offer something: a
    /// surface that has to look for it is a surface that can fail to find it, and an
    /// offer of nothing is still an offer somebody may agree to nothing of.
    pub agreement: String,
    /// What was carried out, in the order it was.
    pub mended: Vec<Mended>,
    /// What has been tried too often to keep offering.
    ///
    /// Said rather than passed over. A repair that quietly stopped being offered leaves
    /// the operator watching a fault nobody mentions any more, which is worse than being
    /// told plainly that this is past what lemonfiber can work out.
    pub beyond: Vec<Beyond>,
    /// Whether this run was allowed to act at all.
    pub acted: bool,
}

/// Offer what can be put right, carry out what is confirmed, and prove each one.
///
/// `confirm` is asked once per repair, and only where the stance says to ask — an
/// unattended run was confirmed by the flag that made it unattended. It is a closure for
/// the reason [`crate::app::forwarding::push`] takes one: the deciding belongs to whichever
/// surface is running this, and the sequence belongs here. Synchronous, because deciding
/// is: a surface that must wait on something to answer can wait before calling this.
///
/// Taken as a trait object rather than by type, so there is one of this function however
/// many surfaces call it — a generic would be copied per call site, and the copy nothing
/// drives is a copy the coverage gate counts and no test can reach.
///
/// `disruptive` belongs to the half that may act. A run that may not is the offer, and an
/// offer is what somebody reads before deciding — so it is refused rather than widened.
///
/// # Errors
///
/// Returns a [`Problem`] where the stack cannot be read, which is the one thing every
/// check needs before any of this can begin, and where a run that may not act was asked
/// for the checks that disturb.
pub async fn mend(
    ctx: &Ctx,
    stance: Stance,
    disruptive: bool,
    confirm: &dyn Confirm,
) -> Result<Report, Box<Problem>> {
    if disruptive && !stance.may_act() {
        return Err(Box::new(offer_cannot_disturb()));
    }
    let (stack, checks) = crate::app::engine::assembled(ctx, disruptive).await?;
    // A second set, assembled without the disruptive ones, for proving the work. Built
    // here rather than per repair: nine checks constructed once are nine constructed once,
    // however many faults this run puts right.
    let (_, again) = crate::app::engine::assembled(ctx, false).await?;
    Ok(mending(
        ctx,
        &stack.manifest.services,
        &checks,
        &again,
        stance,
        confirm,
    )
    .await)
}

/// The same errand, over checks somebody else assembled.
///
/// Apart from [`mend`] so that what a repair *does* can be driven against checks written
/// for the purpose — a runner reachable only through nine real checks and a live stack is
/// one nobody writes a test for, which is how two defects got into this module's first
/// draft.
///
/// `again` is what proves the work, and is a second set rather than the same one: a check
/// holds what it read when it was built, so asking the very instances that found the fault
/// would compare a repair with the reading it was meant to change.
pub async fn mending(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    checks: &[Box<dyn Check>],
    again: &[Box<dyn Check>],
    stance: Stance,
    confirm: &dyn Confirm,
) -> Report {
    let found = looked(ctx, services, checks).await;

    // Remembered before anything is offered. Whether a fix was declined, and how often one
    // has been tried and left the fault standing, are questions about a check across runs —
    // and the store is where lemonfiber keeps that. A run that consulted it without first
    // telling it what this run found would be asking about faults it had never mentioned.
    let conditions = remembered(ctx, &found);
    let proposals = proposed(checks, &found);
    let all: Vec<Repair> = proposals.iter().map(|(_, repair)| repair.clone()).collect();
    let offered = repair::offered(&all, &conditions.all());

    // Asked before the loop rather than inside it, so an offer that has moved on since
    // it was read costs nothing and leaves no record of a decline nobody made.
    let acting = stance.may_act() && confirm.stands(&offered);
    let mut report = Report {
        agreement: repair::agreement(&offered),
        offered: offered.clone(),
        mended: Vec::new(),
        beyond: beyond(&conditions.all(), &all),
        acted: acting,
    };
    if !acting {
        return report;
    }

    // Each survivor keeps the mender that offered it. Looking one up again by check name
    // would need a branch for the case where it is not found — which cannot happen, and so
    // could not be tested, and would have to claim something untrue if it ever ran.
    for (mender, repair) in proposals
        .into_iter()
        .filter(|(_, proposal)| offered.iter().any(|kept| kept.check == proposal.check))
        .filter_map(|(at, proposal)| checks.get(at)?.mender().map(|found| (found, proposal)))
    {
        if stance.asks() && !confirm.agreed(&repair) {
            declined(ctx, &repair);
            report.mended.push(Mended {
                repair,
                outcome: Outcome::Declined,
            });
            continue;
        }
        // Asked before anything is carried out: a repair that must not go ahead is never
        // attempted, rather than attempted and reported as having changed nothing.
        //
        // Matched rather than read through `allowed()`, because the three ways of not
        // going ahead are not one answer: two of them are conclusions about a value and
        // the third is an instruction about an area, and an operator told the wrong one
        // goes looking for a change they did not make. A match the compiler checks is
        // also what makes a fourth answer, if there ever is one, a decision somebody
        // takes here rather than something that quietly joins the refusals.
        let outcome = match permitted(ctx, mender, &repair).await {
            Writing::Ours => carried(ctx, services, mender, again, &repair).await,
            Writing::Unmanaged => Outcome::Unmanaged,
            Writing::Changed | Writing::Adopted | Writing::TheirsAlone => Outcome::WouldOverwrite,
        };
        recorded(ctx, &repair, &outcome);
        report.mended.push(Mended { repair, outcome });
    }
    report
}

/// Whether this repair may go ahead: the declaration asked first, the mender second.
///
/// The declaration comes first because it is the stronger answer and the cheaper one.
/// It is a decision already taken, it needs nothing of a service, and asking the mender
/// first would mean reaching a service the operator told lemonfiber to leave alone in
/// order to find out whether to leave it alone.
///
/// This is the fifth of the five points a declaration has to hold, and the one it could
/// not reach while a repair was only a check name. What a mender would write is now the
/// mender's to declare and nobody else's to guess.
async fn permitted(ctx: &Ctx, mender: &dyn crate::doctor::Mend, repair: &Repair) -> Writing {
    let declared = &ctx.settings.unmanaged;
    if mender
        .writes_to(repair)
        .iter()
        .any(|what| crate::unmanaged::covers(declared, what))
    {
        return Writing::Unmanaged;
    }
    mender.may_proceed(repair).await
}

/// Put back what the last repair changed, and say what went back.
///
/// That repair and no other. The journal it reads is shared with seeding and the first-run
/// wizard, and somebody undoing the thing they just watched happen has not asked for the
/// wiring underneath it to come apart.
///
/// Two reversals, in order: the changes that live inside a service go back through that
/// service, and what is left — settings, directories — goes back on the host. In that
/// order because the service is the part that can be unreachable, and an operator whose
/// Sonarr is down should still get their environment file back.
///
/// # Errors
///
/// Returns a [`Problem`] where the stack cannot be read, where a setting or directory
/// could not be put back, or where a change needed a service that would not answer — the
/// last of which names every such change together rather than one at a time.
pub async fn retract(ctx: &Ctx, paths: &Paths) -> Result<Vec<Undo>, Box<Problem>> {
    let undos = repair::undoing(crate::app::recover::journal_at(&paths.journal()).changes());
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let project =
        crate::app::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let reached =
        crate::app::recover::reconfigured(ctx, &undos, &manifest.services, project.as_deref())
            .await;
    crate::app::recover::undo(&reached.left, &paths.env_file(), reached.unreached)?;
    Ok(undos.into_iter().map(told).collect())
}

pub(crate) use crate::error::codes::repair::NOWHERE_TO_LOOK;

pub use crate::error::codes::repair::OFFER_CANNOT_DISTURB;

pub use crate::app::putting_back::{Left, Reversal};

/// Offer or carry out the repairs, at whatever this run was given consent for.
///
/// The one way in for a run whose consent is settled before it begins, so the
/// mapping from consent to stance is made once rather than per surface. A consent
/// given for an offer that has since moved on is refused here, having carried
/// nothing out.
///
/// What it cannot carry is the question a terminal puts mid-run and waits on, which
/// reaches [`mend`] with an [`Confirm`] of its own.
///
/// # Errors
///
/// Returns a [`Problem`] where the stack cannot be read, or where the offer the
/// consent names is not the offer that stands now.
pub async fn putting_right(
    ctx: &Ctx,
    consent: &Consent,
    disruptive: bool,
) -> Result<Report, Box<Problem>> {
    let report = mend(ctx, consent.stance(), disruptive, consent).await?;
    consent.held(&report)?;
    Ok(report)
}

/// Put back what the last repair changed, from wherever this machine keeps them.
///
/// Apart from [`retract`] by exactly one thing: resolving the layout. A surface
/// that already holds one passes it; one that holds a context alone asks here.
///
/// # Errors
///
/// Returns a [`Problem`] where this machine will not say where lemonfiber's own
/// files are, and every reason [`retract`] gives.
pub async fn reversing(ctx: &Ctx) -> Result<Reversal, Box<Problem>> {
    let paths = crate::app::targets::layout(ctx).ok_or_else(|| Box::new(nowhere_to_look()))?;
    retracting(ctx, &paths).await
}

/// The same, for a surface that already holds the layout — and the one place the
/// rehearsal of it is decided.
///
/// Here rather than in the caller for the reason every other verdict is taken away from
/// the caller: a surface asked to decide can be written without deciding, and what that
/// looks like from outside is the reversal happening and the report calling it a
/// rehearsal. The terminal's `doctor --undo` comes in here rather than through the
/// dispatcher, so without this the one gate the flag has would sit beside the one path
/// that skips it.
///
/// The undos are read out of the journal either way; what a rehearsal leaves out is the
/// two steps that act on them, and what it answers with is the split the named run's
/// rehearsal answers with — through the same function, so the two cannot come to differ.
///
/// # Errors
///
/// Returns a [`Problem`] where the stack cannot be read, where a setting or directory
/// could not be put back, or where a change needed a service that would not answer.
pub async fn retracting(ctx: &Ctx, paths: &Paths) -> Result<Reversal, Box<Problem>> {
    if ctx.dry_run {
        let undos = repair::undoing(crate::app::recover::journal_at(&paths.journal()).changes());
        return Ok(crate::app::putting_back::would_reverse(undos));
    }
    retract(ctx, paths).await.map(|reversed| Reversal {
        reversed,
        left: Vec::new(),
        // Nothing judged, so nothing to say beyond what went back. What a repair puts
        // back is its own work of a moment ago rather than a run somebody named, and
        // the one note that exists is about a setting no repair writes.
        noted: Vec::new(),
        rehearsed: false,
    })
}

/// The refusal for an offer that was asked to include the checks that disturb.
///
/// Both halves of the reason are said, because either alone reads as a rule for its
/// own sake. The offer half of a repair is what an operator reads before deciding,
/// and the checks that disturb prove themselves by disturbing: the killswitch test
/// takes the default route away from the download client, and the release search
/// spends one of the indexers' daily allowance. A run that did either to say what
/// it *would* do has already done it.
///
/// And it costs the operator nothing to be refused. Neither disturbing check can be
/// put right by lemonfiber — no repair answers either of them — so a widened offer
/// offers exactly what the plain one does. What the operator asking for it wants is
/// what those checks *found*, which is a diagnosis, and a diagnosis widened this way
/// is a request this surface already serves.
fn offer_cannot_disturb() -> Problem {
    Problem::new(
        OFFER_CANNOT_DISTURB,
        Severity::Error,
        "Saying what could be put right does not include the checks that disturb",
        "The checks that disturb prove themselves by disturbing: the killswitch test takes \
         the tunnel away from the download client, and the release check spends one of the \
         indexers' daily searches. A run that only says what it would put right has agreed \
         to neither, and neither turns up a repair to offer. Nothing was disturbed.",
        Remedy::new("Ask for the diagnosis with those checks in it, which is what reports them")
            .with_detail("lemonfiber doctor --disruptive"),
    )
    .or_try(Remedy::new(
        "Or agree to the repairs first, and the checks that disturb run with them",
    ))
    .in_state(State::Guided)
}

/// The refusal for a run that cannot say where lemonfiber's own files are.
///
/// The journal is the record of what a repair changed, and a run that cannot name
/// it has nothing to read a reversal out of.
fn nowhere_to_look() -> Problem {
    Problem::new(
        NOWHERE_TO_LOOK,
        Severity::Error,
        "This run has nowhere it knows to look for what a repair changed",
        "What each repair changed is recorded in lemonfiber's own directory, and this \
         machine would not say where that is. Nothing was put back.",
        Remedy::new("Set a home directory for this user and run it again"),
    )
    .in_state(State::Guided)
}

/// Every repair the checks that can mend would offer for what was just found, each kept
/// beside the check that offered it — because a repair names its finding and there is no
/// way back from a finding to the thing that raised it.
fn proposed(checks: &[Box<dyn Check>], found: &[Finding]) -> Vec<(usize, Repair)> {
    checks
        .iter()
        .enumerate()
        .filter_map(|(at, check)| check.mender().map(|mender| (at, mender)))
        .flat_map(|(at, mender)| {
            mender
                .repairs(found)
                .into_iter()
                .map(move |repair| (at, repair))
        })
        .collect()
}

#[cfg(test)]
mod tests;
