//! Driving the wizard one answer at a time, for a surface that does not hold the
//! walk.
//!
//! A terminal keeps the whole conversation in one process: it asks, waits, and
//! asks again, and the wizard lives on its stack for the length of it. A request
//! cannot. So the walk is taken a step per call — where am I, here is one answer,
//! back one, apply — and between calls the accumulated answers live in the one
//! file setup is already allowed to write before review.
//!
//! **The progress file is the state, and nothing else is.** It is what a resumed
//! terminal run reads, so a setup begun in a browser is one a terminal finishes
//! and the other way round, and two windows open on the same machine are looking
//! at one run rather than two. A copy held in the server would die with the
//! process; a copy carried by the caller would be a second store beside this one,
//! and would put every gathered secret back on the wire on every call.
//!
//! Nothing here decides anything the wizard decides. Which question comes next,
//! which apply at all, what an answer may be and what the answers add up to are
//! all read off the wizard; this reads the file, hands the wizard what arrived,
//! and writes the file back.

use crate::app::targets::layout;
use crate::app::Ctx;
use crate::config::paths::Paths;
use crate::config::store;
use crate::error::{Amiss, Code, Diagnose, Problem, Remedy, Severity, State};
use crate::model::{SettingReport, WizardReport};
use crate::validate::{Credential, Validation, Validator};
use crate::wizard::{
    offer_setup, Answer, Choice, Indexer, Phase, Progress, Provider, Status, Wizard,
};

/// What a setup request asks of the wizard.
///
/// The whole of the walk a surface drives, and no more of it: the informing steps
/// are passed with [`Self::Next`] because they have no answer to give, and
/// applying is its own request because it is the one that writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupAction {
    /// Where setup stands. Changes nothing, and refuses nothing.
    Where,
    /// Record one answer, and move on to the next question that applies.
    Answer(Answer),
    /// Move on without answering — how a step that only informs is passed.
    Next,
    /// Move back to the previous question that applies.
    Back,
    /// Write the answers, once every applicable question has one.
    Apply,
    /// Take an apply that stopped part-way the way the operator chose out of it.
    ///
    /// Its own request rather than a kind of apply, because the three ways out are
    /// not three ways of applying: one carries on, one reverses what was written and
    /// applies again, and one reverses it and forgets the answers. What was already
    /// written is on the report, so the choice is made by somebody who has seen it.
    Recover(Choice),
}

/// Carry out one step of setup and say where that leaves it.
///
/// # Errors
///
/// Returns a [`Problem`] where there is nowhere to keep configuration, where this
/// machine is already set up and nothing is part-way through, where the answer does
/// not apply on this platform, or where applying failed — the marker left for
/// recovery in that last case, exactly as a terminal run leaves it.
pub async fn setting_up(ctx: &Ctx, action: SetupAction) -> Result<WizardReport, Box<Problem>> {
    let Some(paths) = layout(ctx) else {
        return Err(Box::new(store::Failure::Nowhere.problem()));
    };
    let saved = super::progress_at(&paths.setup_progress());

    // Asked before anything is read into a wizard: a machine that is already set
    // up has no questions left to put, and answering them again would walk a
    // working stack back to its first one. A read is exempt because whether setup
    // is on offer is precisely what it answers.
    if !matches!(action, SetupAction::Where) && !open(saved.as_ref(), &paths) {
        return Err(Box::new(already_set_up()));
    }

    let mut wizard = saved.map_or_else(
        || Wizard::new(ctx.environment),
        |progress| Wizard::resume(ctx.environment, progress),
    );

    let mut proof = None;
    match action {
        SetupAction::Where => {}
        SetupAction::Answer(answer) => {
            let (answer, came_to) = proven(ctx.validator.as_ref(), answer).await;
            proof = came_to;
            wizard
                .answer(answer)
                .map_err(|rejected| Box::new(super::does_not_apply(rejected)))?;
            wizard.advance();
            super::save(&wizard, &paths);
        }
        SetupAction::Next => {
            wizard.advance();
            super::save(&wizard, &paths);
        }
        SetupAction::Back => {
            wizard.back();
            super::save(&wizard, &paths);
        }
        // The same apply a terminal run reaches, at the same gate: review is
        // entered only from a complete set of answers, and applying anything else
        // is refused there rather than judged again here.
        SetupAction::Apply => super::resume(&mut wizard, &paths, ctx.stack, &ctx.stamp())?,
        // Refused where nothing is part-way through: the three ways out are about a
        // half-written apply, and offering them for a run that has not begun one
        // would reverse changes nothing made and discard answers nobody replaced.
        SetupAction::Recover(choice) => {
            if wizard.phase() != Phase::Applying {
                return Err(Box::new(nothing_to_recover()));
            }
            super::recovered(&mut wizard, &paths, ctx.stack, &ctx.stamp(), choice)?;
            // Starting over forgot the answers, so the walk is back at its
            // beginning — and a report still reading them off the wizard in hand
            // would describe a run that no longer exists anywhere.
            if choice == Choice::StartOver {
                wizard = Wizard::new(ctx.environment);
            }
        }
    }

    Ok(reported(&wizard, &paths, proof))
}

/// Whether setup may still be answered or applied on this machine.
///
/// A run to pick up beats a machine that looks configured, and is asked first for
/// that reason: an apply that stopped part-way has written settings that the
/// configured-yet question reads as a finished install.
fn open(saved: Option<&Progress>, paths: &Paths) -> bool {
    Status::of(saved).unfinished() || offer_setup(paths.env_file().exists())
}

/// Where the walk stands now, read off the wizard and the files rather than
/// remembered.
///
/// The progress is read back rather than taken from the wizard in hand, because a
/// finished apply removes it — so what this reports is what the next run will
/// find, which is the thing a surface is deciding on.
fn reported(wizard: &Wizard, paths: &Paths, proof: Option<Validation>) -> WizardReport {
    let saved = super::progress_at(&paths.setup_progress());
    WizardReport {
        proof,
        written: if wizard.phase() == Phase::Applying {
            super::written_so_far(paths)
        } else {
            Vec::new()
        },
        offered: open(saved.as_ref(), paths),
        phase: wizard.phase(),
        at: wizard.at(),
        asks: wizard.at().is_question(),
        unanswered: wizard.unanswered(),
        ready_for_review: wizard.ready_for_review(),
        // Withheld the way `config show` withholds, through the same rule: this is
        // the one place setup's answers are read back out, and an indexer key or a
        // provider password in it would be one a caller could log.
        plan: wizard
            .plan()
            .settings()
            .iter()
            .map(|(key, value)| SettingReport::from(store::showing(key, value)))
            .collect(),
    }
}

/// The answer as this path may record it, and what proving it came to.
///
/// A credential is tested against its live service before it is kept, the way a
/// terminal run tests one the moment it is entered — so `validated` records what a
/// test established rather than what a caller asserted. Taken as submitted it would
/// let a caller claim a key works by saying it does, and a later diagnosis reads
/// that flag to decide whether to trust one.
///
/// A test that does not prove it is not a refusal. The answer is kept unproven and
/// what the service said comes back beside it, which leaves whoever asked the three
/// ways out a terminal run offers: send a different credential, send none, or go on
/// with one that is recorded as unverified.
///
/// Every other answer is about this machine rather than about a service, so there is
/// nothing to ask and nothing comes back.
async fn proven(validator: &dyn Validator, answer: Answer) -> (Answer, Option<Validation>) {
    match answer {
        Answer::Credentials(Some(indexer)) => {
            let came_to = validator
                .validate(&Credential::Indexer {
                    url: indexer.url.clone(),
                    key: indexer.key.clone(),
                })
                .await;
            let validated = matches!(came_to, Validation::Valid { .. });
            (
                Answer::Credentials(Some(Indexer {
                    validated,
                    ..indexer
                })),
                Some(came_to),
            )
        }
        Answer::Provider(Some(provider)) => {
            let came_to = validator
                .validate(&Credential::Usenet {
                    host: provider.host.clone(),
                    port: provider.port,
                    secure: provider.tls,
                    user: provider.user.clone(),
                    pass: provider.pass.clone(),
                })
                .await;
            let validated = matches!(came_to, Validation::Valid { .. });
            (
                Answer::Provider(Some(Provider {
                    validated,
                    ..provider
                })),
                Some(came_to),
            )
        }
        other => (other, None),
    }
}

/// The problem of recovering an apply that is not part-way through.
fn nothing_to_recover() -> Problem {
    Problem::new(
        NOTHING_TO_RECOVER,
        Severity::Error,
        "No setup here stopped part-way through applying",
        "Recovering chooses what to do about a half-written apply, and there is none to \
         choose about. Nothing has been changed.",
        Remedy::new("Ask where setup stands before offering a way out of it"),
    )
    .in_state(State::Guided)
    .lies_in(Amiss::Asking)
}

/// Raised when a recovery is asked for and no apply stopped part-way.
pub const NOTHING_TO_RECOVER: Code = Code::new("SETUP-8");

/// The problem of answering setup on a machine that already holds configuration.
fn already_set_up() -> Problem {
    Problem::new(
        ALREADY_SET_UP,
        Severity::Error,
        "This machine is already set up",
        "Setup asks what a fresh install needs to be told, so answering it again would walk a working stack back to its first question. Nothing has been changed.",
        Remedy::new("Change a setting rather than running setup again")
            .with_detail("lemonfiber config set <key> <value>"),
    )
    .in_state(State::Guided)
    .lies_in(Amiss::Asking)
}

/// Raised when setup is answered on a machine that is already set up.
pub const ALREADY_SET_UP: Code = Code::new("SETUP-7");

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use async_trait::async_trait;

    use super::{setting_up, SetupAction, ALREADY_SET_UP, NOTHING_TO_RECOVER};
    use crate::alert::Appetite;
    use crate::app::apply::NOT_REVIEWED;
    use crate::app::setup::DOES_NOT_APPLY;
    use crate::app::Ctx;
    use crate::config::paths::Paths;
    use crate::config::{
        store, Protocols, Settings, INDEXER_APIKEY_KEY, INDEXER_URL_KEY, PROVIDER_PASS_KEY,
    };
    use crate::error::Code;
    use crate::journal::{Change, Kind};
    use crate::model::WizardReport;
    use crate::stack::Source;
    use crate::test_support::a_context;
    use crate::validate::{Credential, Validation, Validator};
    use crate::wizard::{
        Answer, Choice, Indexer, Library, Phase, Progress, Provider, Step, Vpn, Wizard,
    };

    /// A scratch layout unique to this process and case, cleared first.
    fn scratch(name: &str) -> Paths {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-walk-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Paths::rooted(&dir.join("config"), &dir.join("data"))
    }

    /// A validator that answers what it was built with, whatever it is asked.
    ///
    /// What this path has to be shown is that a credential records what the service
    /// said rather than what the caller claimed, which needs the outcome and not
    /// the service that produced one.
    struct Saying(Validation);

    #[async_trait]
    impl Validator for Saying {
        async fn validate(&self, _credential: &Credential) -> Validation {
            self.0.clone()
        }
    }

    /// What a service that proved the credential answered.
    fn proven() -> Arc<dyn Validator> {
        Arc::new(Saying(Validation::Valid {
            observed: "answered a search — 40 results".to_owned(),
        }))
    }

    /// What a service that would not take the credential answered.
    fn turned_away() -> Arc<dyn Validator> {
        Arc::new(Saying(Validation::Rejected {
            detail: "the indexer answered 401".to_owned(),
        }))
    }

    /// A context keeping its files in that layout, on a platform where the
    /// container user is never asked about.
    ///
    /// The stack is one already on disk, so applying materialises nothing and what
    /// is being driven is the walk rather than a stack being written out.
    fn ctx(paths: &Paths) -> Ctx {
        proving(paths, proven())
    }

    /// The same, proving credentials through a given validator.
    fn proving(paths: &Paths, validator: Arc<dyn Validator>) -> Ctx {
        a_context()
            .over(Source::External(Path::new("/lemonfiber-not-a-real-stack")))
            .settings(Settings {
                env_file: Some(paths.env_file()),
                stack_dir: Some(paths.stack()),
                ..Settings::default()
            })
            .build()
            .proving(validator)
    }

    /// Where a step of the walk left setup, or nothing where it refused.
    async fn walked(ctx: &Ctx, action: SetupAction) -> Option<WizardReport> {
        setting_up(ctx, action).await.ok()
    }

    /// Which refusal a step of the walk met, or nothing where it met none.
    async fn refused(ctx: &Ctx, action: SetupAction) -> Option<Code> {
        setting_up(ctx, action)
            .await
            .err()
            .map(|problem| problem.code)
    }

    /// A value that must not be printed back, assembled rather than written out so
    /// no credential sits in this source.
    fn withheld_value(word: &str) -> String {
        [word, "not", "real"].join("-")
    }

    /// The value a report's plan carries for a setting, or nothing where it holds
    /// none — and nothing too where the step itself refused.
    fn planned(report: Option<&WizardReport>, key: &str) -> Option<String> {
        report?
            .plan
            .iter()
            .find(|setting| setting.key == key)
            .map(|setting| setting.value.clone())
    }

    /// An indexer credential, as a caller submits one.
    fn an_indexer(validated: bool) -> Answer {
        Answer::Credentials(Some(Indexer {
            url: "http://indexer.invalid/api".to_owned(),
            key: withheld_value("indexer"),
            validated,
        }))
    }

    /// A Usenet provider, as a caller submits one.
    fn a_provider(validated: bool) -> Answer {
        Answer::Provider(Some(Provider {
            host: "news.invalid".to_owned(),
            port: 563,
            user: "someone".to_owned(),
            pass: withheld_value("provider"),
            tls: true,
            validated,
        }))
    }

    /// Every answer this platform asks for, in order.
    fn all_of_them(root: &Path) -> [Answer; 9] {
        [
            Answer::Protocols(Protocols::both()),
            Answer::Vpn(Vpn::Carrying),
            Answer::DataLocation(root.to_path_buf()),
            Answer::Credentials(None),
            Answer::Provider(None),
            Answer::Library(Library::None),
            Answer::Household(false),
            Answer::Notifications(Appetite::ProblemsOnly),
            Answer::Autostart(false),
        ]
    }

    /// Answer every question, one request each, as a surface would.
    async fn answer_everything(ctx: &Ctx, root: &Path) {
        for answer in all_of_them(root) {
            assert!(setting_up(ctx, SetupAction::Answer(answer)).await.is_ok());
        }
    }

    /// What an interrupted apply leaves behind: half-written settings, the marker
    /// saying the writing had begun, and the journal of what it managed to write.
    fn interrupted(paths: &Paths) {
        assert!(store::write(&paths.env_file(), "DATA_ROOT=/srv\n").is_ok());
        let stopped = Progress {
            phase: Phase::Applying,
            ..Progress::default()
        };
        assert!(store::write(
            &paths.setup_progress(),
            &serde_json::to_string(&stopped).unwrap_or_default()
        )
        .is_ok());
        let written = Change {
            at: "1".to_owned(),
            operation: "setup".to_owned(),
            target: ".env".to_owned(),
            kind: Kind::Set {
                key: "DATA_ROOT".to_owned(),
                previous: None,
                current: "/srv".to_owned(),
            },
        };
        assert!(store::write(
            &paths.journal(),
            &serde_json::to_string(&written).unwrap_or_default()
        )
        .is_ok());
    }

    #[tokio::test]
    async fn a_fresh_machine_is_offered_setup_and_stands_at_its_first_step() {
        let paths = scratch("fresh");
        let report = walked(&ctx(&paths), SetupAction::Where).await;

        assert_eq!(report.as_ref().map(|report| report.at), Some(Step::Welcome));
        assert_eq!(
            report.as_ref().map(|report| report.asks),
            Some(false),
            "the welcome only informs"
        );
        assert_eq!(
            report.as_ref().map(|report| report.offered),
            Some(true),
            "there is no configuration to protect"
        );
        assert_eq!(
            report.as_ref().map(|report| report.ready_for_review),
            Some(false),
            "nothing is answered yet"
        );
        assert_eq!(
            report.map(|report| report.written.len()),
            Some(0),
            "and no apply stopped part-way"
        );
        assert!(
            !paths.setup_progress().exists(),
            "asking where setup stands writes nothing"
        );
    }

    #[tokio::test]
    async fn a_step_that_only_informs_is_passed_without_an_answer() {
        let paths = scratch("informing");
        let report = walked(&ctx(&paths), SetupAction::Next).await;

        assert_eq!(report.map(|report| report.at), Some(Step::Preflight));
        assert!(
            paths.setup_progress().exists(),
            "where the walk reached survives quitting"
        );
    }

    #[tokio::test]
    async fn an_answer_is_recorded_and_the_walk_moves_on() {
        let paths = scratch("recorded");
        let context = ctx(&paths);
        let report = walked(
            &context,
            SetupAction::Answer(Answer::Protocols(Protocols::both())),
        )
        .await;

        assert_eq!(
            report
                .as_ref()
                .map(|report| report.unanswered.contains(&Step::Protocols)),
            Some(false),
            "the question it answered is no longer outstanding"
        );
        assert_eq!(
            report.as_ref().map(|report| report.at),
            Some(Step::Preflight),
            "and the walk has moved on"
        );
        assert_eq!(
            report.map(|report| report.proof),
            Some(None),
            "an answer about this machine has no service to prove it against"
        );
        // Read back through a second request, which is the point of the file: this
        // surface holds nothing between one call and the next.
        assert_eq!(
            walked(&context, SetupAction::Where)
                .await
                .map(|report| report.unanswered.contains(&Step::Protocols)),
            Some(false)
        );
    }

    #[tokio::test]
    async fn going_back_returns_to_the_previous_step_that_applies() {
        let paths = scratch("back");
        let context = ctx(&paths);

        assert_eq!(
            walked(&context, SetupAction::Next).await.map(|r| r.at),
            Some(Step::Preflight)
        );
        assert_eq!(
            walked(&context, SetupAction::Back).await.map(|r| r.at),
            Some(Step::Welcome)
        );
    }

    #[tokio::test]
    async fn an_answer_this_platform_does_not_offer_is_refused_and_nothing_is_kept() {
        let paths = scratch("rejected");
        // Ownership is mapped away on this platform, so a container user would have
        // no observable effect and the wizard refuses to record one.
        let met = refused(
            &ctx(&paths),
            SetupAction::Answer(Answer::ServiceUser(Some((1000, 1000)))),
        )
        .await;

        assert_eq!(met, Some(DOES_NOT_APPLY));
        assert!(
            !paths.setup_progress().exists(),
            "an answer that was refused is not one that was saved"
        );
    }

    #[tokio::test]
    async fn a_credential_records_what_the_service_said_and_says_what_that_was() {
        let paths = scratch("proven");
        let context = ctx(&paths);
        assert!(setting_up(
            &context,
            SetupAction::Answer(Answer::Protocols(Protocols::both()))
        )
        .await
        .is_ok());

        // Submitted claiming nothing, and proven anyway, because what is recorded is
        // what the live test established rather than what arrived.
        let report = walked(&context, SetupAction::Answer(an_indexer(false))).await;

        assert_eq!(
            planned(report.as_ref(), "INDEXER_VALIDATED").as_deref(),
            Some("on"),
            "the indexer answered"
        );
        assert_eq!(
            report.and_then(|report| report.proof),
            Some(Validation::Valid {
                observed: "answered a search — 40 results".to_owned()
            }),
            "and what it did is said, not merely that it answered"
        );
    }

    #[tokio::test]
    async fn a_credential_the_service_will_not_take_is_kept_unproven_and_said_so() {
        let paths = scratch("unproven");
        let context = proving(&paths, turned_away());
        assert!(setting_up(
            &context,
            SetupAction::Answer(Answer::Protocols(Protocols::both()))
        )
        .await
        .is_ok());

        // Both arrive asserting they were proven, and the service refused both.
        assert!(setting_up(&context, SetupAction::Answer(an_indexer(true)))
            .await
            .is_ok());
        let report = walked(&context, SetupAction::Answer(a_provider(true))).await;

        assert_eq!(
            planned(report.as_ref(), "INDEXER_VALIDATED").as_deref(),
            Some("off"),
            "saying a key works is not proving it"
        );
        assert_eq!(
            planned(report.as_ref(), "USENET_VALIDATED").as_deref(),
            Some("off"),
            "nor the login"
        );
        assert_eq!(
            report.and_then(|report| report.proof),
            Some(Validation::Rejected {
                detail: "the indexer answered 401".to_owned()
            }),
            "and the refusal is carried back rather than swallowed"
        );
    }

    #[tokio::test]
    async fn entering_no_credential_at_all_asks_nothing_of_any_service() {
        let paths = scratch("none-entered");
        let context = ctx(&paths);
        assert!(setting_up(
            &context,
            SetupAction::Answer(Answer::Protocols(Protocols::both()))
        )
        .await
        .is_ok());

        let report = walked(&context, SetupAction::Answer(Answer::Credentials(None))).await;

        assert_eq!(
            report.map(|report| report.proof),
            Some(None),
            "there was nothing to prove"
        );
    }

    #[tokio::test]
    async fn what_was_entered_is_never_repeated_back() {
        let paths = scratch("withholding");
        let context = ctx(&paths);
        assert!(setting_up(
            &context,
            SetupAction::Answer(Answer::Protocols(Protocols::both()))
        )
        .await
        .is_ok());
        assert!(setting_up(&context, SetupAction::Answer(an_indexer(false)))
            .await
            .is_ok());
        let report = walked(&context, SetupAction::Answer(a_provider(false))).await;

        assert_eq!(
            planned(report.as_ref(), INDEXER_APIKEY_KEY).as_deref(),
            Some(store::REDACTED)
        );
        assert_eq!(
            planned(report.as_ref(), PROVIDER_PASS_KEY).as_deref(),
            Some(store::REDACTED)
        );
        // The address beside the key is not itself a secret, and a review that hid
        // it would be hiding what the operator is there to check.
        assert_eq!(
            planned(report.as_ref(), INDEXER_URL_KEY).as_deref(),
            Some("http://indexer.invalid/api")
        );
        let rendered = report
            .as_ref()
            .and_then(|report| serde_json::to_string(report).ok())
            .unwrap_or_default();
        assert!(
            !rendered.contains(&withheld_value("indexer"))
                && !rendered.contains(&withheld_value("provider")),
            "neither reaches anything a caller could log: {rendered}"
        );
    }

    #[tokio::test]
    async fn applying_before_every_question_is_answered_is_refused() {
        let paths = scratch("early");

        assert_eq!(
            refused(&ctx(&paths), SetupAction::Apply).await,
            Some(NOT_REVIEWED)
        );
        assert!(!paths.env_file().exists(), "and nothing was written");
    }

    #[tokio::test]
    async fn a_complete_set_of_answers_is_written_and_setup_stops_being_offered() {
        let paths = scratch("applied");
        let context = ctx(&paths);
        let root = paths.data_dir().join("media");
        answer_everything(&context, &root).await;

        let report = walked(&context, SetupAction::Apply).await;

        assert_eq!(
            report.as_ref().map(|report| report.phase),
            Some(Phase::Applied)
        );
        assert_eq!(
            report.map(|report| report.offered),
            Some(false),
            "this machine is set up now"
        );
        assert!(paths.env_file().exists(), "the settings landed");
        assert!(root.is_dir(), "and so did the library's home");
        assert!(
            !paths.setup_progress().exists(),
            "the resumable copy of the answers is not left lying about"
        );
    }

    #[tokio::test]
    async fn a_machine_already_set_up_is_told_so_rather_than_asked_again() {
        let paths = scratch("configured");
        let context = ctx(&paths);
        assert!(store::write(&paths.env_file(), "DATA_ROOT=/srv\n").is_ok());

        assert_eq!(
            refused(
                &context,
                SetupAction::Answer(Answer::Protocols(Protocols::both()))
            )
            .await,
            Some(ALREADY_SET_UP)
        );
        // Asking is still answered: whether setup is on offer is exactly what a
        // surface asks this to decide whether to show the wizard at all.
        assert_eq!(
            walked(&context, SetupAction::Where)
                .await
                .map(|report| report.offered),
            Some(false)
        );
    }

    #[tokio::test]
    async fn an_apply_that_stopped_part_way_is_picked_up_rather_than_read_as_finished() {
        let paths = scratch("interrupted");
        interrupted(&paths);

        let report = walked(&ctx(&paths), SetupAction::Where).await;

        assert_eq!(
            report.as_ref().map(|report| report.offered),
            Some(true),
            "a half-written apply is not a finished install"
        );
        assert_eq!(
            report.as_ref().map(|report| report.phase),
            Some(Phase::Applying),
            "and it says which of the two it is"
        );
        assert_eq!(
            report.map(|report| report.written),
            Some(vec!["the setting DATA_ROOT".to_owned()]),
            "and names what it had already written, so a choice is made about it"
        );
    }

    #[tokio::test]
    async fn starting_over_undoes_what_was_written_and_forgets_the_answers() {
        let paths = scratch("start-over");
        interrupted(&paths);

        let report = walked(&ctx(&paths), SetupAction::Recover(Choice::StartOver)).await;

        assert_eq!(
            report.as_ref().map(|report| report.at),
            Some(Step::Welcome),
            "the walk is back at its beginning"
        );
        assert_eq!(
            report.map(|report| report.written.len()),
            Some(0),
            "and there is nothing left to choose about"
        );
        assert!(!paths.setup_progress().exists(), "the answers are gone");
        assert!(
            !paths.journal().exists(),
            "and so is the record of the apply"
        );
        assert_eq!(
            store::read(&paths.env_file())
                .ok()
                .and_then(|settings| settings.get("DATA_ROOT").map(ToOwned::to_owned)),
            None,
            "and the setting it had written is off the file"
        );
    }

    #[tokio::test]
    async fn rolling_back_undoes_what_was_written_and_applies_the_answers_again() {
        let paths = scratch("roll-back");
        let context = ctx(&paths);
        let root = paths.data_dir().join("media");
        answer_everything(&context, &root).await;
        // An apply that stopped after writing one setting, over the answers just
        // gathered — which is the only state a roll back is offered from.
        let saved = super::super::progress_at(&paths.setup_progress());
        let mut progress = saved.unwrap_or_default();
        progress.phase = Phase::Applying;
        assert!(store::write(
            &paths.setup_progress(),
            &serde_json::to_string(&progress).unwrap_or_default()
        )
        .is_ok());

        let report = walked(&context, SetupAction::Recover(Choice::RollBack)).await;

        assert_eq!(
            report.map(|report| report.phase),
            Some(Phase::Applied),
            "the answers were applied again"
        );
        assert!(paths.env_file().exists(), "and the settings landed");
    }

    #[tokio::test]
    async fn resuming_carries_the_apply_forward_from_the_answers_it_kept() {
        let paths = scratch("resume-recovery");
        let context = ctx(&paths);
        let root = paths.data_dir().join("media");
        answer_everything(&context, &root).await;
        let saved = super::super::progress_at(&paths.setup_progress());
        let mut progress = saved.unwrap_or_default();
        progress.phase = Phase::Applying;
        assert!(store::write(
            &paths.setup_progress(),
            &serde_json::to_string(&progress).unwrap_or_default()
        )
        .is_ok());

        let report = walked(&context, SetupAction::Recover(Choice::Resume)).await;

        assert_eq!(report.map(|report| report.phase), Some(Phase::Applied));
        assert!(paths.env_file().exists());
    }

    #[tokio::test]
    async fn a_way_out_of_an_apply_that_never_stopped_is_refused() {
        let paths = scratch("nothing-to-recover");
        let context = ctx(&paths);
        assert!(setting_up(
            &context,
            SetupAction::Answer(Answer::Protocols(Protocols::both()))
        )
        .await
        .is_ok());

        assert_eq!(
            refused(&context, SetupAction::Recover(Choice::StartOver)).await,
            Some(NOTHING_TO_RECOVER)
        );
        assert!(
            paths.setup_progress().exists(),
            "and the answers it would have forgotten are still there"
        );
    }

    #[tokio::test]
    async fn nowhere_to_keep_configuration_is_said_rather_than_guessed_at() {
        let nowhere = a_context().settings(Settings::default()).build();

        assert!(
            setting_up(&nowhere, SetupAction::Where).await.is_err(),
            "a run with no configured home has nowhere to gather answers into"
        );
    }

    #[tokio::test]
    async fn the_walk_settles_on_what_the_wizard_itself_would() {
        // The whole claim of this module: it drives the wizard rather than deciding
        // anything of its own, so what it comes to is what the wizard comes to.
        let paths = scratch("agrees");
        let context = ctx(&paths);
        let root = paths.data_dir().join("media");
        answer_everything(&context, &root).await;

        let mut wizard = Wizard::new(context.environment);
        for answer in all_of_them(&root) {
            assert!(wizard.answer(answer).is_ok());
        }

        let over_requests: Vec<(String, String)> = walked(&context, SetupAction::Where)
            .await
            .map(|report| {
                report
                    .plan
                    .into_iter()
                    .map(|setting| (setting.key, setting.value))
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(over_requests, wizard.plan().settings().to_vec());
        assert!(
            over_requests.contains(&("DATA_ROOT".to_owned(), root.display().to_string())),
            "and it is not an empty agreement: {over_requests:?}"
        );
    }
}
