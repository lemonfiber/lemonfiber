//! Moving the stack onto the versions this build pins.
//!
//! Two answers from one request. Unconfirmed it says what would change — the version
//! each service stands on, the version it would move to, how large the step is, and
//! that a step which migrates state is one nothing walks back. Confirmed it takes
//! those steps, one service at a time, behind a backup.
//!
//! The order the confirmed half runs in is the whole of its safety, and each part of
//! it exists because the one before it can fail: what is still downloading is asked
//! about before anything stops, the disk is measured before anything is pulled, the
//! backup is taken before anything opens its state on a newer binary, and each
//! service is proven to answer before the next one is touched. A run that failed
//! part-way says exactly which services moved and which did not, because the
//! alternative is an operator guessing at a half-migrated stack.
//!
//! What each of those steps *is* lives where it already lived — the transfers with
//! the teardown that first had to ask about them, the disk with the reckoning, the
//! capture with the archive. Nothing here is a second implementation of any of them.

mod staging;

use lemonfiber_manifest::Manifest;
use serde::Serialize;

use crate::error::{Code, Diagnose, Problem, Remedy, Severity, State};
use crate::migration::{pins, Ours};
use crate::model::StackEdit;
use crate::plural::s;
use crate::update::{self, Applied, Change, State as Standing};

use super::engine::{in_flight, Interrupted};
use super::{Ctx, Waiting};

/// Raised when what this machine has pulled could not be read.
pub const NOT_CHECKED: Code = Code::new("UPDATE-1");

/// Raised when the service an update was narrowed to is not one the stack declares.
pub const NO_SUCH_SERVICE: Code = Code::new("UPDATE-2");

/// Raised when transfers are still in flight and the run was not asked to wait.
pub const STILL_TRANSFERRING: Code = Code::new("UPDATE-3");

/// Raised when the stack came down for the capture and the capture would not write.
pub const CAPTURE_LEFT_IT_DOWN: Code = Code::new("UPDATE-4");

/// What was asked of an update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    /// The one service to move, or every one of them where absent.
    pub service: Option<String>,
    /// Whether the operator agreed to the steps being taken.
    pub confirm: bool,
    /// Whether anything still downloading is let finish first.
    pub wait: Waiting,
}

/// What updating the stack would change, or what a run of it came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Report {
    /// The one word the run comes to.
    pub state: Standing,
    /// What would move, and what taking each step means.
    pub changes: Vec<Change>,
    /// What the download clients are still working on, named so an operator can
    /// tell whether the thing they have been waiting for is among them.
    pub in_flight: Vec<String>,
    /// Whether the steps were agreed to, or only shown.
    pub confirmed: bool,
    /// Where the backup taken before anything moved was written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<String>,
    /// Stack files the operator had edited, left as they set them rather than
    /// overwritten with this build's own, each with the change that was held back.
    pub stack_edits: Vec<StackEdit>,
    /// What became of each service the run reached, in the order it reached them.
    pub applied: Vec<Applied>,
    /// Why the stack is not as the run found it, where it is not.
    ///
    /// Two runs end that way and an operator has the same thing to do about either:
    /// one that met a service which would not come back and stopped there, and one
    /// where every step succeeded and the stack would not start again afterwards.
    /// The second is not a failure of the update — `state` still says `Updated`,
    /// because it is — but the stack came down for the capture and something has to
    /// say that it is still down.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub halted: Option<String>,
}

impl Report {
    /// What a run that changed nothing answers with.
    #[must_use]
    fn proposed(changes: Vec<Change>, in_flight: Vec<String>, confirmed: bool) -> Self {
        Self {
            state: update::state(&changes, &[]),
            changes,
            in_flight,
            confirmed,
            backup: None,
            stack_edits: Vec::new(),
            applied: Vec::new(),
            halted: None,
        }
    }
}

/// What updating would change, and — only when confirmed — the change itself.
///
/// # Errors
///
/// Returns a [`Problem`] where the stack cannot be read, where the service named is
/// not one it declares, where the engine will not say what it has pulled, and for
/// any reason the confirmed run refuses to proceed.
pub(super) async fn update(ctx: &Ctx, asked: Asked) -> Result<Report, Box<Problem>> {
    // The pins come from the manifest, which is validated as it is read — so a stack
    // declaring a tag that moves under it is refused here rather than being updated
    // to whatever that tag pointed at today.
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let moving = narrowed(&manifest, asked.service.as_deref())?;

    let images = ctx
        .images
        .images()
        .await
        .map_err(|_| Box::new(not_checked()))?;
    let changes = update::changes(&moving, &images, &ctx.settings.project);

    if asked.confirm {
        return staging::apply(ctx, &manifest, changes, asked.wait).await;
    }
    let active = waiting(ctx, &changes).await;
    Ok(Report::proposed(changes, active, false))
}

/// The services this run is about: the one named, or every one the stack declares.
///
/// A name the stack does not declare is refused rather than quietly matching
/// nothing, which would read as a stack already up to date.
fn narrowed(manifest: &Manifest, service: Option<&str>) -> Result<Vec<Ours>, Box<Problem>> {
    let every = pins(manifest);
    let Some(named) = service else {
        return Ok(every);
    };
    let only: Vec<Ours> = every
        .into_iter()
        .filter(|pin| pin.service == named)
        .collect();
    if only.is_empty() {
        return Err(Box::new(no_such_service(named, manifest)));
    }
    Ok(only)
}

/// What the download clients are working on, where anything would move at all.
///
/// Asked only when there is a step to take. A stack already on every pin has nothing
/// to warn anybody about, and going to two clients to establish that would be this
/// command making work out of an answer of "nothing".
async fn waiting(ctx: &Ctx, changes: &[Change]) -> Vec<String> {
    if changes.is_empty() {
        return Vec::new();
    }
    named(&in_flight(ctx, &[]).await)
}

/// What is in flight, as an operator would read it.
fn named(active: &[Interrupted]) -> Vec<String> {
    active
        .iter()
        .map(|one| format!("{} ({}%)", one.name, one.progress))
        .collect()
}

/// The refusal for an engine that will not say what it has pulled.
fn not_checked() -> Problem {
    Problem::new(
        NOT_CHECKED,
        Severity::Error,
        "What is running could not be read, so no update was worked out",
        "Which version each service stands on is the engine's answer, and it did not give one. \
         Nothing was pulled, started or stopped.",
        Remedy::new("Start the container engine, then ask again"),
    )
    .in_state(State::Guided)
}

/// The refusal for a service the stack does not declare.
fn no_such_service(named: &str, manifest: &Manifest) -> Problem {
    let declared: Vec<&str> = manifest
        .services
        .iter()
        .map(|service| service.id.as_str())
        .collect();
    Problem::new(
        NO_SUCH_SERVICE,
        Severity::Error,
        format!("`{named}` is not a service this stack declares"),
        "An update narrowed to one service has to name one, and a name that matches nothing \
         would read as a stack already up to date.",
        Remedy::new("Name one of the services the stack declares").with_detail(declared.join(", ")),
    )
    .in_state(State::Guided)
}

/// The refusal for a capture that would not write, once the stack is already down.
///
/// A capture is refused while anything might be writing, so the stack is stopped
/// before one is attempted. That makes the capture the first thing in the run whose
/// failure leaves the machine somewhere the operator did not put it, and the backup's
/// own words are about the archive rather than about the stack — true, and not the
/// part that needs acting on. So what stopped the capture is carried as the cause and
/// the state of the stack leads.
fn left_down(cause: Problem) -> Problem {
    Problem::new(
        CAPTURE_LEFT_IT_DOWN,
        Severity::Error,
        "the stack was stopped for the backup, and the backup would not write",
        "Nothing was updated and nothing opened its state on a newer image, so there is \
         nothing to undo. The stack is down, because it was stopped so the capture could \
         run with nothing writing to a database.",
        Remedy::new("Bring the stack back up").with_detail("lemonfiber up"),
    )
    .or_try(Remedy::new(
        "Then fix what stopped the capture and ask for the update again",
    ))
    .caused_by(cause)
}

/// The refusal for a run that would interrupt what is still coming down.
///
/// Refused rather than reported and carried out. The wait is the offer, and an
/// offer nothing stops the run from walking past is not one — a torrent at ninety-four
/// per cent does not resume where it left off on every tracker, and the operator who
/// would mind is the one who cannot see this list from the command they typed.
fn still_transferring(active: &[String]) -> Problem {
    let count = active.len();
    Problem::new(
        STILL_TRANSFERRING,
        Severity::Warning,
        format!("{count} download{} still coming down", s(count)),
        "Updating stops the download clients, and what is part-way down does not always resume \
         where it left off. Nothing has been pulled, stopped or changed.",
        Remedy::new("Let them finish first, then update")
            .with_detail("lemonfiber update --confirm --wait"),
    )
    .in_state(State::Guided)
    .with_detail(active.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::{named, narrowed, still_transferring, update, Asked, Report};
    use crate::app::engine::Interrupted;
    use crate::app::{Ctx, Waiting};
    use crate::dashboard::Protocol;
    use crate::test_support::{a_context, nowhere};
    use crate::update::State as Standing;
    use lemonfiber_fixtures::pulled::Pulled;

    /// What was asked, with nothing narrowed and nothing agreed to.
    fn asking() -> Asked {
        Asked {
            service: None,
            confirm: false,
            wait: Waiting::Never,
        }
    }

    /// A run against an engine that has pulled nothing.
    ///
    /// Named rather than left to the default, which is this machine's own daemon: a
    /// test that read a real engine would answer differently on every machine it ran
    /// on, and would answer nothing at all where no daemon is installed.
    fn nothing_pulled() -> Ctx {
        a_context().build().with_images(Pulled::holding(Vec::new()))
    }

    /// The manifest the shipped stack declares.
    fn manifest() -> Option<lemonfiber_manifest::Manifest> {
        let ctx = a_context().build();
        let today = ctx.today();
        ctx.stack.checked_manifest(today).ok()
    }

    #[tokio::test]
    async fn a_stack_whose_engine_answers_nothing_is_already_on_its_pins() {
        let ctx = nothing_pulled();
        let read = update(&ctx, asking()).await;
        let state = read.map(|report| (report.state, report.changes.len(), report.confirmed));
        assert_eq!(state.ok(), Some((Standing::Current, 0, false)));
    }

    #[tokio::test]
    async fn a_stack_that_cannot_be_read_is_refused_rather_than_reported_as_current() {
        let ctx = a_context()
            .over(nowhere())
            .build()
            .with_images(Pulled::holding(Vec::new()));
        let read = update(&ctx, asking()).await;
        assert!(
            read.is_err(),
            "a stack nothing could read said what it holds"
        );
    }

    #[tokio::test]
    async fn an_engine_that_will_not_say_what_it_pulled_stops_the_reading() {
        let ctx = a_context()
            .build()
            .with_images(Pulled::unreachable("no daemon here"));
        let read = update(&ctx, asking()).await;
        let code = read.err().map(|problem| problem.code.to_string());
        assert_eq!(code.as_deref(), Some("UPDATE-1"));
    }

    #[tokio::test]
    async fn a_service_the_stack_does_not_declare_is_refused_by_name() {
        let ctx = nothing_pulled();
        let read = update(
            &ctx,
            Asked {
                service: Some("not-a-service".to_owned()),
                ..asking()
            },
        )
        .await;
        let code = read.err().map(|problem| problem.code.to_string());
        assert_eq!(code.as_deref(), Some("UPDATE-2"));
    }

    #[test]
    fn narrowing_to_one_service_leaves_the_rest_of_the_stack_alone() {
        let manifest = manifest();
        let only = manifest
            .as_ref()
            .and_then(|manifest| narrowed(manifest, Some("sonarr")).ok())
            .unwrap_or_default();
        let names: Vec<String> = only.into_iter().map(|pin| pin.service).collect();
        assert_eq!(names, vec!["sonarr".to_owned()]);
    }

    #[test]
    fn narrowing_to_nothing_takes_every_service_the_stack_declares() {
        let manifest = manifest();
        let every = manifest
            .as_ref()
            .and_then(|manifest| narrowed(manifest, None).ok())
            .unwrap_or_default();
        let count = every.len();
        assert!(count > 10, "{count} services");
    }

    #[test]
    fn what_is_in_flight_is_named_with_how_far_along_it_is() {
        let active = [Interrupted {
            protocol: Protocol::Torrent,
            name: "Ubuntu.iso".to_owned(),
            progress: 94,
        }];
        assert_eq!(named(&active), vec!["Ubuntu.iso (94%)".to_owned()]);
    }

    #[test]
    fn a_run_that_would_interrupt_a_transfer_says_how_to_let_it_finish() {
        let problem = still_transferring(&["Ubuntu.iso (94%)".to_owned()]);
        let remedy = problem.remedies.first().and_then(|one| one.detail.clone());
        assert_eq!(problem.code.to_string(), "UPDATE-3");
        assert_eq!(
            remedy.as_deref(),
            Some("lemonfiber update --confirm --wait")
        );
    }

    #[test]
    fn a_proposal_carries_nothing_a_run_would_have_left_behind() {
        let report = Report::proposed(Vec::new(), Vec::new(), false);
        assert_eq!(report.clone(), report);
        assert_eq!(report.backup, None);
        assert_eq!(report.halted, None);
        assert!(report.applied.is_empty());
    }
}
