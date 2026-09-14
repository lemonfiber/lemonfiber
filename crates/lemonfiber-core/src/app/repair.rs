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

mod consent;
mod proving;
mod remembering;
mod telling;

pub use consent::{Consent, STALE};
use proving::{carried, looked};
use remembering::{beyond, declined, recorded, remembered};
pub(in crate::app) use telling::told;

use crate::config::paths::Paths;
use crate::doctor::{Check, Finding};
use crate::error::{Code, Diagnose as _, Problem, Remedy, Severity, State};
use crate::journal::Undo;
use crate::repair::{self, Outcome, Repair, Stance, Writing};

use super::Ctx;

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
/// the reason [`super::forwarding::push`] takes one: the deciding belongs to whichever
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
    let (manifest, checks) = super::engine::assembled(ctx, disruptive).await?;
    // A second set, assembled without the disruptive ones, for proving the work. Built
    // here rather than per repair: nine checks constructed once are nine constructed once,
    // however many faults this run puts right.
    let (_, again) = super::engine::assembled(ctx, false).await?;
    Ok(mending(ctx, &manifest.services, &checks, &again, stance, confirm).await)
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
    let undos = repair::undoing(super::recover::journal_at(&paths.journal()).changes());
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let project = super::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let reached =
        super::recover::reconfigured(ctx, &undos, &manifest.services, project.as_deref()).await;
    super::recover::undo(&reached.left, &paths.env_file(), reached.unreached)?;
    Ok(undos.into_iter().map(told).collect())
}

/// Raised when a run cannot say where lemonfiber's own files are.
pub const NOWHERE_TO_LOOK: Code = Code::new("REPAIR-2");

/// Raised when a run that may not act was asked for the checks that disturb.
pub const OFFER_CANNOT_DISTURB: Code = Code::new("REPAIR-3");

pub use super::putting_back::{Left, Reversal};

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
    let paths = super::targets::layout(ctx).ok_or_else(|| Box::new(nowhere_to_look()))?;
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
        let undos = repair::undoing(super::recover::journal_at(&paths.journal()).changes());
        return Ok(super::putting_back::would_reverse(undos));
    }
    retract(ctx, paths).await.map(|reversed| Reversal {
        reversed,
        left: Vec::new(),
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
mod tests {
    use super::proving::{judged, proved};
    use super::remembering::wrong;
    use super::{
        beyond, mending, putting_right, reversing, Beyond, Confirm, Consent, NOWHERE_TO_LOOK,
    };
    use crate::app::fixtures::ctx_at;
    use crate::condition::{Conditions, Fault};
    use crate::doctor::{Category, Check, Finding, Mend, Verdict};
    use crate::error::{Code, Problem, Remedy, Severity};
    use crate::repair::{Attempt, Outcome, Repair, Stance, ATTEMPTS};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    fn problem() -> Problem {
        Problem::new(
            Code::new("VPN-7"),
            Severity::Warning,
            "it is on the wrong port",
            "peers cannot reach it",
            Remedy::new("move it"),
        )
    }

    fn finding(check: &str, verdict: Verdict) -> Finding {
        Finding::in_category(Category::Vpn, check, "the forwarded port", verdict)
    }

    fn repair(check: &str) -> Repair {
        Repair {
            check: check.to_owned(),
            does: "move the client".to_owned(),
            effects: Vec::new(),
            reversible: false,
        }
    }

    /// What the check below says is wrong, and what the repair it offers is named for.
    ///
    /// A check written for the purpose rather than one of the real ones: what is under
    /// test is the sequence's handling of a declaration, and reaching that through nine
    /// real checks and a live stack is a test nobody writes.
    const MENDABLE: &str = "test.always-wrong";

    /// A mender that says what it would write, and records whether it was ever asked
    /// to write it.
    ///
    /// Both halves are load-bearing. The declaration is what the gate above it reads.
    /// The record is how "nothing was written" is proved as a fact about the mender
    /// rather than inferred from the report — a run that wrote and then said
    /// `Unmanaged` would carry exactly the report this one does.
    struct Writes {
        /// What a repair here would write to, by the names a declaration uses.
        areas: Vec<String>,
        /// Set the moment it is asked to carry a repair out.
        asked: Arc<AtomicBool>,
    }

    impl Writes {
        /// A mender declaring these areas, having been asked to write nothing yet.
        fn to(areas: &[&str]) -> Self {
            Self {
                areas: areas.iter().map(|area| (*area).to_owned()).collect(),
                asked: Arc::new(AtomicBool::new(false)),
            }
        }

        /// The flag it sets when asked to write, kept by the test after the mender
        /// itself has been handed to the runner.
        fn record(&self) -> Arc<AtomicBool> {
            Arc::clone(&self.asked)
        }
    }

    #[async_trait::async_trait]
    impl Mend for Writes {
        fn repairs(&self, found: &[Finding]) -> Vec<Repair> {
            found
                .iter()
                .filter(|finding| matches!(finding.verdict, Verdict::Warn(_) | Verdict::Fail(_)))
                .map(|finding| repair(&finding.check))
                .collect()
        }

        async fn mend(&self, _repair: &Repair) -> Attempt {
            self.asked.store(true, Ordering::Relaxed);
            Attempt::carried()
        }

        fn writes_to(&self, _repair: &Repair) -> Vec<String> {
            self.areas.clone()
        }
    }

    /// A check that always finds the one fault that mender answers for, so a run
    /// driven here always has something to offer.
    struct Offering(Writes);

    #[async_trait::async_trait]
    impl Check for Offering {
        fn category(&self) -> Category {
            Category::Vpn
        }

        async fn run(&self) -> Vec<Finding> {
            vec![finding(MENDABLE, Verdict::Warn(problem()))]
        }

        fn mender(&self) -> Option<&dyn Mend> {
            Some(&self.0)
        }
    }

    /// Agrees to whatever it is asked about, and leaves the question of whether the
    /// offer still stands to the answer the trait gives when nobody overrides it.
    ///
    /// Which is the answer a surface that never read an offer has to give. A terminal
    /// asks about each repair in the same run that looked, so there is no earlier
    /// offer for this one to have moved on from — only consent that crossed a request
    /// boundary has one, and only that has a reason to say no here.
    struct Agreeing;

    impl Confirm for Agreeing {
        fn agreed(&self, _repair: &Repair) -> bool {
            true
        }
    }

    /// A consent that read no offer says the offer in front of it still stands.
    ///
    /// The answer the trait gives where nobody overrides it, which is the whole reason
    /// it has one: a terminal asks about each repair in the same run that looked, so
    /// there is no earlier offer for this one to have moved on from. Only consent that
    /// crossed a request boundary read an offer this run has since looked again for,
    /// and only that has anything to say no about. Asserted rather than left to the
    /// default's obviousness, because the day somebody gives the trait a second
    /// implementor that forgets to override it is the day a stale answer is carried
    /// out against an offer nobody is making any more.
    #[test]
    fn a_consent_that_read_no_offer_says_the_offer_in_front_of_it_still_stands() {
        assert!(
            Agreeing.stands(&[]),
            "a run that looked and asked in one go has no older offer to have moved on \
             from"
        );
    }

    /// The whole sequence over one check, with the mender the caller keeps a record of.
    async fn driven(ctx: &crate::app::Ctx, mender: Writes) -> (super::Report, Arc<AtomicBool>) {
        let wrote = mender.record();
        let checks: Vec<Box<dyn Check>> = vec![Box::new(Offering(mender))];
        let report = mending(ctx, &[], &checks, &checks, Stance::Ask, &Agreeing).await;
        (report, wrote)
    }

    /// A context with these areas declared unmanaged.
    fn declaring(name: &str, areas: &[(&str, &str)]) -> crate::app::Ctx {
        let mut ctx = ctx_at(name);
        ctx.settings.unmanaged = areas
            .iter()
            .map(|(area, why)| ((*area).to_owned(), (*why).to_owned()))
            .collect();
        ctx
    }

    /// The fifth write point. A repair is the one write an operator asks for by name,
    /// and it is still not a way past a declaration they made.
    #[tokio::test]
    async fn a_repair_that_would_write_a_declared_area_is_refused_by_the_declaration() {
        let ctx = declaring(
            "repair-unmanaged",
            &[("sonarr", "I tune this one by hand every season")],
        );
        let mender = Writes::to(&["sonarr"]);

        let standing = super::permitted(&ctx, &mender, &repair("wiring.sonarr")).await;

        assert_eq!(standing, crate::repair::Writing::Unmanaged);
        assert!(!standing.allowed());
        // And the sentence it carries is about the instruction they gave rather than
        // about a change they made, which is the whole reason it is its own answer.
        let said = standing.refused().map(|remedy| remedy.action.clone());
        assert!(
            said.is_some_and(|said| said.contains("declared this unmanaged")),
            "{standing:?}"
        );
    }

    /// A name beneath a declared one is covered by it, the way every other write point
    /// reads a declaration.
    #[tokio::test]
    async fn a_repair_writing_beneath_a_declared_area_is_refused_too() {
        let ctx = declaring("repair-unmanaged-beneath", &[("config", "all mine")]);
        let mender = Writes::to(&["config/recyclarr/recyclarr.yml"]);

        let standing = super::permitted(&ctx, &mender, &repair("quality.preset")).await;

        assert_eq!(standing, crate::repair::Writing::Unmanaged);
    }

    /// And where nothing is declared, the mender is asked exactly as before — the
    /// default answer being that a repair touching nothing declarable may proceed.
    #[tokio::test]
    async fn a_repair_touching_nothing_declared_is_left_to_the_mender() {
        let ctx = declaring("repair-unmanaged-elsewhere", &[("radarr", "mine as well")]);
        let mender = Writes::to(&["sonarr"]);

        let standing = super::permitted(&ctx, &mender, &repair("wiring.sonarr")).await;

        assert_eq!(standing, crate::repair::Writing::Ours);
        assert!(standing.allowed());
    }

    /// A mender that writes nothing an operator could have declared theirs is never
    /// held by a declaration, whatever they wrote down.
    #[tokio::test]
    async fn a_repair_that_declares_no_write_is_not_held_by_anything() {
        let ctx = declaring("repair-unmanaged-nothing", &[("sonarr", "mine")]);
        let mender = Writes::to(&[]);

        let standing = super::permitted(&ctx, &mender, &repair("engine.restart")).await;

        assert_eq!(standing, crate::repair::Writing::Ours);
    }

    /// A declaration reaches all the way through the sequence: the repair is agreed
    /// to, refused by the declaration, reported as refused, and never carried out.
    ///
    /// The gate itself is tested above, which says it answers correctly. This says the
    /// runner acts on that answer, and says it twice over. The outcome is `Unmanaged`
    /// rather than the answer given for a value somebody changed — an operator told the
    /// wrong one goes looking for a change they did not make. And the mender is asked
    /// whether it was ever asked to write, because a run that wrote and then reported
    /// `Unmanaged` would carry exactly the report this one does.
    #[tokio::test]
    async fn a_repair_the_declaration_refuses_is_reported_as_such_and_never_carried_out() {
        let ctx = declaring(
            "repair-mending-unmanaged",
            &[("sonarr", "I tune this one by hand every season")],
        );

        let (report, wrote) = driven(&ctx, Writes::to(&["sonarr"])).await;

        assert_eq!(
            report.mended.first().map(|mended| &mended.outcome),
            Some(&Outcome::Unmanaged),
            "{report:?}"
        );
        assert!(
            !wrote.load(Ordering::Relaxed),
            "the mender was asked to write an area the operator declared theirs"
        );
    }

    /// And a declaration about somewhere else stops nothing: the same repair, the same
    /// agreement, and the mender is asked to carry it out.
    ///
    /// The half that keeps the rule honest. A gate that refused everything would pass
    /// the test above and be useless, and an operator who declared one service theirs
    /// has said nothing about the rest of their stack.
    #[tokio::test]
    async fn a_repair_writing_outside_every_declared_area_is_carried_out() {
        let ctx = declaring("repair-mending-elsewhere", &[("radarr", "mine as well")]);

        let (report, wrote) = driven(&ctx, Writes::to(&["sonarr"])).await;

        assert!(
            wrote.load(Ordering::Relaxed),
            "the mender was never asked to write: {report:?}"
        );
        assert_ne!(
            report.mended.first().map(|mended| &mended.outcome),
            Some(&Outcome::Unmanaged),
            "{report:?}"
        );
    }

    /// What a run found has to reach the store before anything is offered, or every
    /// question asked of it — declined? tried too often? — is asked about a check it has
    /// never heard of, and answered no.
    #[test]
    fn what_a_run_found_is_remembered_before_anything_is_offered() {
        let ctx = ctx_at("repair-remembers");
        let found = vec![
            finding("vpn.port-forward-client", Verdict::Warn(problem())),
            finding("vpn.egress", Verdict::Pass { note: None }),
        ];

        let conditions = super::remembered(&ctx, &found);

        assert!(conditions
            .get("vpn.port-forward-client")
            .is_some_and(crate::condition::Condition::is_raised));
        // A check that has never been wrong gets no entry at all. The store remembers
        // faults, and inventing a cleared condition for something that never failed would
        // fill it with things that never happened — which is also why a repair asks it
        // about the checks that did fail and no others.
        assert!(conditions.get("vpn.egress").is_none());

        // And it survives to the next run, which is the whole point of a store.
        assert!(super::super::conditions::load(&ctx)
            .get("vpn.port-forward-client")
            .is_some());
    }

    /// A rehearsal reads the store and does not add to it.
    ///
    /// What this file holds is how often a fault has been seen and how often a fix left
    /// it standing, and the offer decides what is worth offering again from those
    /// counts. A rehearsal that recorded a sighting would move them, so the next real
    /// run would decide differently because somebody had asked a question.
    #[test]
    fn a_rehearsed_repair_reads_the_store_and_writes_nothing_to_it() {
        let ctx = ctx_at("repair-rehearsed").rehearsing();
        let found = vec![finding("vpn.port-forward-client", Verdict::Warn(problem()))];

        let conditions = super::remembered(&ctx, &found);

        // The report a rehearsal gives is built from the reading, so the reading happens.
        assert!(conditions
            .get("vpn.port-forward-client")
            .is_some_and(crate::condition::Condition::is_raised));
        // Against the real file, not against what the function said it did.
        let kept = crate::app::fixtures::scratch("repair-rehearsed").join("conditions.json");
        assert!(!kept.exists(), "a rehearsal wrote {}", kept.display());
        assert!(super::super::conditions::load(&ctx)
            .get("vpn.port-forward-client")
            .is_none());
    }

    /// A pass says nothing is wrong and a skip says there was nothing to look at. An
    /// unverified check is the careful one: it could not be established, which is not the
    /// same as finding it broken.
    #[test]
    fn only_a_finding_that_says_something_is_wrong_is_remembered_as_a_fault() {
        assert!(wrong(&finding("a", Verdict::Warn(problem()))).is_some());
        assert!(wrong(&finding("a", Verdict::Fail(problem()))).is_some());
        assert!(wrong(&finding("a", Verdict::Pass { note: None })).is_none());
        assert!(wrong(&finding(
            "a",
            Verdict::Skipped {
                reason: "nothing to look at".to_owned()
            }
        ))
        .is_none());
        assert!(wrong(&finding(
            "a",
            Verdict::Unverified {
                reason: "could not be established".to_owned(),
                remedy: Remedy::new("try again"),
            }
        ))
        .is_none());
    }

    /// All three answers the proof can give, and what each makes of an attempt that ran.
    #[test]
    fn an_attempt_and_its_proof_together_say_what_happened() {
        assert_eq!(judged(Attempt::carried(), Some(true)), Outcome::Fixed);
        assert_eq!(judged(Attempt::carried(), Some(false)), Outcome::FixFailed);
        // Could not be established afterwards: neither fixed nor demonstrably still
        // broken, and reported as what it is rather than as the worse of the two.
        assert!(matches!(
            judged(Attempt::carried(), None),
            Outcome::Stopped { .. }
        ));
        // One that stopped is not judged by the proof at all — what it left behind is
        // what the operator needs, whatever the checks say now.
        assert_eq!(
            judged(
                Attempt::Stopped {
                    leaving: "half of it".to_owned()
                },
                Some(true)
            ),
            Outcome::Stopped {
                leaving: "half of it".to_owned()
            }
        );
    }

    /// The question a repair is judged by. Absent means the fault is gone; unverified means
    /// nobody can say, which must not be mistaken for either answer.
    #[test]
    fn whether_a_check_now_passes_has_three_answers() {
        let check = "vpn.port-forward-client";
        assert_eq!(proved(&[], check), Some(true));
        assert_eq!(
            proved(&[finding(check, Verdict::Pass { note: None })], check),
            Some(true)
        );
        assert_eq!(
            proved(&[finding(check, Verdict::Warn(problem()))], check),
            Some(false)
        );
        assert_eq!(
            proved(
                &[finding(
                    check,
                    Verdict::Unverified {
                        reason: "could not be established".to_owned(),
                        remedy: Remedy::new("try again"),
                    }
                )],
                check
            ),
            None
        );
    }

    /// Said only where something could still have been offered, and only while the fault is
    /// actually standing — clearing a condition does not reset its attempts, so a fault that
    /// went away would otherwise still be announced as beyond repair.
    #[test]
    fn only_a_standing_fault_with_a_repair_is_reported_as_beyond_one() {
        let mut conditions = Conditions::new();
        let fault = Fault::new("vpn.port", Severity::Warning, "wrong port", "move it");
        conditions.observe("vpn.port-forward-client", Some(&fault), "1000");
        for _ in 0..ATTEMPTS {
            conditions.attempted("vpn.port-forward-client");
        }

        let named = |beyond: Vec<Beyond>| -> Vec<String> {
            beyond.into_iter().map(|one| one.check).collect()
        };

        assert_eq!(
            named(beyond(
                &conditions.all(),
                &[repair("vpn.port-forward-client")]
            )),
            vec!["vpn.port-forward-client".to_owned()]
        );

        // Nothing could have repaired it, so nothing has run out of ways to.
        assert!(named(beyond(&conditions.all(), &[])).is_empty());

        // And once it clears, it is not something that outlasted its repairs any more.
        conditions.observe("vpn.port-forward-client", None, "2000");
        assert!(named(beyond(
            &conditions.all(),
            &[repair("vpn.port-forward-client")]
        ))
        .is_empty());
    }

    /// A run given no consent offers what it found and puts none of it right, which
    /// is what a surface asks for before it has anything to show anybody.
    #[tokio::test]
    async fn a_run_with_no_consent_offers_and_acts_on_none_of_it() {
        // Read as one value rather than unwrapped through a branch nothing takes:
        // nothing is wrong on this machine that lemonfiber could put right, so the
        // offer is empty — and an empty offer still names itself, because consent
        // to nothing is a thing somebody can give.
        let offer = putting_right(&ctx_at("repair-offering"), &Consent::Offer, false)
            .await
            .ok()
            .map(|report| (report.acted, report.offered.len(), report.agreement));

        assert_eq!(offer, Some((false, 0, crate::repair::agreement(&[]))));
    }

    /// A run that may not act is refused the checks that disturb, before anything
    /// is assembled to run.
    ///
    /// The tunnel staying up is proved from `tests/`, over an engine that records
    /// what it was asked to do. What is proved here is that the refusal comes first:
    /// this context has no container engine behind it, so a run that reached the
    /// checks at all would answer with something else entirely.
    #[tokio::test]
    async fn an_offer_asked_to_disturb_is_refused_before_a_check_is_built() {
        let refused = putting_right(&ctx_at("repair-offer-disturbing"), &Consent::Offer, true)
            .await
            .err()
            .map(|problem| (problem.code, problem.remedies.len()));

        // Two remedies, because there are two different things the caller might
        // have wanted: the checks' findings, which is a diagnosis, or the repairs
        // carried out, which is the yes.
        assert_eq!(refused, Some((super::OFFER_CANNOT_DISTURB, 2)));
    }

    /// Consent given for an offer that is not the offer that stands is refused, and
    /// nothing is carried out — which is the whole of what a request boundary costs
    /// a flow that a terminal gets for nothing.
    #[tokio::test]
    async fn consent_given_for_an_offer_that_has_moved_on_is_refused() {
        let consent = Consent::Given {
            offer: "deadbeef".to_owned(),
            repairs: vec!["vpn.port-forward-client".to_owned()],
        };
        let refused = putting_right(&ctx_at("repair-stale"), &consent, false)
            .await
            .err()
            .map(|problem| problem.code);

        assert_eq!(refused, Some(super::STALE));
    }

    /// A run that cannot say where lemonfiber keeps its own files has nothing to
    /// read a reversal out of, and says so rather than reporting that there was
    /// nothing to put back — which is what a machine with a clean journal says.
    #[tokio::test]
    async fn an_undo_with_nowhere_to_look_says_so_rather_than_finding_nothing() {
        let refused = reversing(&ctx_at("repair-nowhere"))
            .await
            .err()
            .map(|problem| problem.code);

        assert_eq!(refused, Some(NOWHERE_TO_LOOK));
    }

    /// With a layout to read, the reversal is the one [`super::retract`] gives, in
    /// the report an envelope carries.
    #[tokio::test]
    async fn an_undo_that_knows_where_to_look_answers_with_what_went_back() {
        let dir = std::env::temp_dir().join(format!(
            "lemonfiber-reversing-{}-{}",
            std::process::id(),
            "layout"
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(dir.join("config"));
        let _ = std::fs::create_dir_all(dir.join("data"));
        let settings = crate::config::Settings {
            env_file: Some(dir.join("config").join(".env")),
            stack_dir: Some(dir.join("data").join("stack")),
            ..crate::config::Settings::default()
        };
        let ctx = crate::test_support::a_context().settings(settings).build();

        // Nothing has been repaired here, so there is nothing to put back — said as
        // an empty reversal rather than as a failure.
        let reversal = reversing(&ctx).await.ok().map(|it| it.reversed.len());

        assert_eq!(reversal, Some(0));
    }
}
