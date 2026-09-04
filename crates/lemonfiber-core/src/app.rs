//! The one way in.
//!
//! A surface turns input into a [`Command`], hands it to [`dispatch`], and
//! renders the [`Outcome`]. A keypress, a subcommand and an HTTP route all
//! become the same value, so the three surfaces cannot grow behaviour that only
//! one of them has — which is the drift that otherwise happens quietly, one
//! convenience flag at a time.
//!
//! Whether this is a rehearsal is a property of the [`Ctx`], not a second code
//! path, so there is no parallel implementation to fall out of step.

use crate::doctor::Narrowing;
use crate::error::{Code, Diagnose, Problem};
use crate::glossary::{Term, Vocabulary};
use crate::model::{
    kind, ConfigReport, DoctorReport, Envelope, FormsReport, FrontDoorReport, HouseholdReport,
    LifecycleReport, MusicReport, QualityReport, ResetReport, StatusReport, StuckReport,
    SupervisionReport, TraceReport, UpgradeReport, VersionReport, WalkthroughReport, WizardReport,
};
use crate::stack::closure::Plan;
use crate::stack::compose::Action;

pub mod accepted;
pub mod appetite;
pub mod apply;
pub mod archives;
pub mod backup;
pub mod bundle;
mod command;
pub mod conditions;
mod configuring;
mod ctx;
pub mod dashboard;
mod door;
pub mod egress;
mod engine;
#[cfg(test)]
mod fixtures;
pub mod forwarding;
mod household;
mod invite;
mod materialise;
mod music;
mod notify;
mod outbox;
mod quality;
pub mod queue;
mod quiesced;
mod record;
pub mod recover;
mod remove;
pub mod repair;
mod repairs;
mod reset;
pub mod restore;
mod screen;
mod seed;
pub mod seeding;
pub mod setup;
mod space;
mod stored;
pub mod support;
mod targets;
mod trace;
mod upgrade;
mod walkthrough;
pub mod watch;

pub use command::{Allowance, Command, QualityAction};
pub use ctx::Ctx;
pub use setup::SetupAction;

// The log-following reads a surface streams from live outside dispatch, so they are the
// engine module's functions re-exported for the binary and the log commands to reach.
pub use engine::{
    claimed, diagnose, in_flight, logs, pull_progress, released, start_progress, started, Claim,
    Interrupted, Waiting,
};
pub use notify::{notify, Notified, CHANNEL_CHECK};
pub use walkthrough::{walkthrough, worth_offering};

// The data-location watch is a self-contained feature in its own module; these
// are the names the rest of the crate and the binary reach it by.
pub use watch::{supervise, ALREADY_GONE, NOTHING_TO_WATCH, WATCH};

/// What dispatching produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The answer to [`Command::Version`].
    Version(VersionReport),
    /// The answer to [`Command::Forms`].
    Forms(FormsReport),
    /// The answer to [`Command::Preview`].
    Preview(Plan),
    /// What a lifecycle command did, or would have done.
    Lifecycle(LifecycleReport),
    /// The answer to a configuration command.
    Config(ConfigReport),
    /// The quality choice, what it means, and what a command did with it.
    Quality(QualityReport),
    /// What upgrading existing content did, or would do, and its stated cost.
    Upgrade(UpgradeReport),
    /// The music format chosen, and what became of applying it.
    Music(MusicReport),
    /// Where one item is in the pipeline.
    Trace(TraceReport),
    /// What the household asked for, member by member.
    Household(HouseholdReport),
    /// The one address to hand somebody who lives here.
    FrontDoor(FrontDoorReport),
    /// The items whose downloads are stuck, each linkable to its trace.
    Stuck(StuckReport),
    /// What one of this product's words means.
    Word(Term),
    /// Every word this product explains.
    Glossary(Vocabulary),
    /// Which app to use on which device.
    Clients(crate::clients::Guidance),
    /// An account offered to somebody in the house.
    Invited(crate::model::Invitation),
    /// Somebody taken out of the household, or what taking them would cost.
    Removed(crate::model::HouseholdRemoval),
    /// Everything that leaves this machine, and what refusing each of them costs.
    Outbound(crate::outbound::Leaving),
    /// Everything this machine keeps of lemonfiber's, and what became of it.
    Stored(crate::stored::Stored),
    /// Where the disk stands, where the room went, and what could be got back.
    Space(crate::space::Reckoning),
    /// What each service is doing.
    Status(StatusReport),
    /// What the diagnostic checks found.
    Doctor(DoctorReport),
    /// What could be put right, and what became of the ones agreed to.
    Repair(repair::Report),
    /// What putting back the last repair came to.
    Undo(repair::Reversal),
    /// What seeding wired, and what it left for a re-run.
    Seed(crate::seed::Report),
    /// What a full reset did, or would do — the operator edits reverted to lemonfiber's.
    Reset(ResetReport),
    /// Where setup stands, and what it is still asking for.
    Wizard(WizardReport),
    /// Where a backup archive was written, and what it covers.
    Backup(backup::Report),
    /// What a support bundle would hold, or where one went.
    Support(support::Bundle),
    /// The backup archives this machine has kept.
    Archives(archives::Listing),
    /// What a restore would overwrite, or what it put back.
    Restore(restore::Restoration),
    /// How a guard ended, and whether it got the services stopped.
    Watch(SupervisionReport),
    /// How far a walk got, and what it proved.
    Walkthrough(WalkthroughReport),
}

impl Outcome {
    /// Wrap this outcome for machine-readable output.
    #[must_use]
    pub fn envelope(self) -> Envelope<Self> {
        let kind = match self {
            Self::Version(_) => kind::VERSION,
            Self::Forms(_) => kind::FORMS,
            Self::Preview(_) => crate::model::kind::PREVIEW,
            Self::Lifecycle(_) => crate::model::kind::LIFECYCLE,
            Self::Config(_) => crate::model::kind::CONFIG,
            Self::Quality(_) => kind::QUALITY,
            Self::Upgrade(_) => kind::UPGRADE,
            Self::Music(_) => kind::MUSIC,
            Self::Trace(_) => kind::TRACE,
            Self::Household(_) => kind::HOUSEHOLD,
            Self::FrontDoor(_) => kind::FRONT_DOOR,
            Self::Stuck(_) => kind::STUCK,
            Self::Word(_) => kind::WORD,
            Self::Glossary(_) => kind::GLOSSARY,
            Self::Clients(_) => kind::CLIENTS,
            Self::Invited(_) => kind::INVITATION,
            Self::Removed(_) => kind::REMOVAL,
            Self::Outbound(_) => crate::model::kind::OUTBOUND,
            Self::Stored(_) => crate::model::kind::STORED,
            Self::Space(_) => kind::SPACE,
            Self::Status(_) => crate::model::kind::STATUS,
            Self::Doctor(_) => kind::DOCTOR,
            Self::Repair(_) => kind::REPAIR,
            Self::Undo(_) => kind::UNDO,
            Self::Seed(_) => kind::SEED,
            Self::Reset(_) => kind::RESET,
            Self::Wizard(_) => kind::WIZARD,
            Self::Backup(_) => kind::BACKUP,
            Self::Support(_) => kind::BUNDLE,
            Self::Archives(_) => kind::ARCHIVES,
            Self::Restore(_) => kind::RESTORE,
            Self::Watch(_) => kind::WATCH,
            Self::Walkthrough(_) => kind::WALKTHROUGH,
        };
        Envelope::new(kind, self)
    }
}

impl serde::Serialize for Outcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Version(report) => report.serialize(serializer),
            Self::Forms(report) => report.serialize(serializer),
            Self::Preview(plan) => plan.serialize(serializer),
            Self::Lifecycle(report) => report.serialize(serializer),
            Self::Config(report) => report.serialize(serializer),
            Self::Quality(report) => report.serialize(serializer),
            Self::Upgrade(report) => report.serialize(serializer),
            Self::Music(report) => report.serialize(serializer),
            Self::Trace(report) => report.serialize(serializer),
            Self::Household(report) => report.serialize(serializer),
            Self::FrontDoor(report) => report.serialize(serializer),
            Self::Stuck(report) => report.serialize(serializer),
            Self::Word(term) => term.serialize(serializer),
            Self::Glossary(report) => report.serialize(serializer),
            Self::Clients(report) => report.serialize(serializer),
            Self::Invited(report) => report.serialize(serializer),
            Self::Removed(report) => report.serialize(serializer),
            Self::Outbound(report) => report.serialize(serializer),
            Self::Stored(report) => report.serialize(serializer),
            Self::Space(report) => report.serialize(serializer),
            Self::Status(report) => report.serialize(serializer),
            Self::Doctor(report) => report.serialize(serializer),
            Self::Repair(report) => report.serialize(serializer),
            Self::Undo(report) => report.serialize(serializer),
            Self::Seed(report) => report.serialize(serializer),
            Self::Reset(report) => report.serialize(serializer),
            Self::Wizard(report) => report.serialize(serializer),
            Self::Backup(report) => report.serialize(serializer),
            Self::Support(report) => report.serialize(serializer),
            Self::Archives(listing) => listing.serialize(serializer),
            Self::Restore(report) => report.serialize(serializer),
            Self::Watch(report) => report.serialize(serializer),
            Self::Walkthrough(report) => report.serialize(serializer),
        }
    }
}

/// Raised when a service never reached a state that starting could accept.
pub const NEVER_SETTLED: Code = Code::new("LIFE-1");

/// Raised when stopping would take a service out from under a form still running.
pub const STILL_NEEDED: Code = Code::new("LIFE-2");

/// Another run is already working on this stack.
pub const ALREADY_WORKING: Code = Code::new("LIFE-3");

/// Fetching images is switched off, so there was nothing to fetch with.
pub const REGISTRY_REFUSED: Code = Code::new("LIFE-4");

/// Ask the engine to act on a set of services, which three commands do identically.
///
/// Named apart because they differ only in the action, and three arms that said the same
/// thing three times is what left `dispatch` with no room for a new command.
async fn acting(ctx: &Ctx, forms: &[String], action: Action) -> Result<Outcome, Box<Problem>> {
    engine::lifecycle(ctx, forms, &action).await
}

/// Offer somebody an account, which is the one request this table takes apart.
///
/// Named apart for the reason [`acting`] is: every other row here passes what it was
/// given straight along in one line, and an invitation carries three things. Spelling
/// them out in the table would make the request this file does least with the longest
/// arm in it.
async fn invited(ctx: &Ctx, name: String, allowance: Allowance) -> Result<Outcome, Box<Problem>> {
    invite::offer(ctx, name, allowance)
        .await
        .map(Outcome::Invited)
}

/// What the services themselves reach out for, which needs the stack read first.
///
/// Named apart for the reason [`acting`] is: every other row here passes what it was
/// given straight along, and this one has a step before it. The stack has to be
/// readable, because half the answer is about it: a manifest that could not be read
/// would leave the services' own requests reading as none at all, which is a claim
/// rather than a gap.
fn outbound(ctx: &Ctx) -> Result<Outcome, Box<Problem>> {
    let manifest = ctx
        .stack
        .manifest()
        .map_err(|err| Box::new(err.problem()))?;
    Ok(Outcome::Outbound(crate::outbound::leaving(
        &ctx.settings,
        &manifest.services,
    )))
}

/// A diagnosis, and the warning it was told to consider answered.
///
/// Named apart for the reason [`acting`] is: the run happens first and what it found
/// is then read against what was accepted, which is two steps rather than a
/// pass-along.
async fn diagnosed(
    ctx: &Ctx,
    narrowing: Narrowing,
    disruptive: bool,
    accept: Option<String>,
) -> Result<Outcome, Box<Problem>> {
    let report = engine::diagnose(ctx, &narrowing, disruptive).await?;
    accepted::acknowledge(ctx, accept.as_deref(), report).map(Outcome::Doctor)
}

/// A walk through the stack, said onto whatever the surface is listening with.
async fn walked(ctx: &Ctx, item: Option<String>) -> Result<Outcome, Box<Problem>> {
    walkthrough::walkthrough(ctx, item.as_deref(), ctx.steps.as_ref())
        .await
        .map(Outcome::Walkthrough)
}

/// Carry out a command.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render when the command could not
/// be carried out.
pub async fn dispatch(command: Command, ctx: &Ctx) -> Result<Outcome, Box<Problem>> {
    match command {
        Command::Version => engine::version(ctx).await.map(Outcome::Version),
        Command::Forms => engine::forms(ctx).map(Outcome::Forms),
        Command::Preview { forms } => engine::preview(ctx, &forms).map(Outcome::Preview),
        Command::Up { forms } => engine::lifecycle(ctx, &forms, &Action::Up).await,
        Command::Start { forms, services } => acting(ctx, &forms, Action::Start(services)).await,
        Command::Down { forms, wait } => engine::teardown(ctx, &forms, wait).await,
        Command::Halt { forms, services } => acting(ctx, &forms, Action::Stop(services)).await,
        Command::Switch { forms } => engine::switch(ctx, &forms).await,
        Command::Restart { forms, services } => {
            acting(ctx, &forms, Action::Restart(services)).await
        }
        Command::Pull { forms } => engine::lifecycle(ctx, &forms, &Action::Pull).await,
        Command::ConfigGet { key } => configuring::configuration(ctx, Some(&key), None),
        Command::ConfigSet { key, value } => {
            configuring::configuration(ctx, Some(&key), Some(&value))
        }
        Command::ConfigShow => configuring::configuration(ctx, None, None),
        Command::Quality(action) => quality::quality(ctx, action).map(Outcome::Quality),
        Command::QualityMusic { format } => music::music(ctx, format).await.map(Outcome::Music),
        Command::Trace {
            term,
            season,
            searching,
        } => trace::trace(ctx, &term, season, searching)
            .await
            .map(Outcome::Trace),
        Command::Household { member } => household::household(ctx, member.as_deref())
            .await
            .map(Outcome::Household),
        Command::FrontDoor => door::front_door(ctx).await.map(Outcome::FrontDoor),
        Command::Stuck => trace::stuck(ctx).await.map(Outcome::Stuck),
        Command::Explain { word } => crate::glossary::explain(&word)
            .map(|term| Outcome::Word(*term))
            .ok_or_else(|| Box::new(crate::glossary::unrecognised(&word))),
        Command::Glossary => Ok(Outcome::Glossary(crate::glossary::vocabulary())),
        Command::Clients => Ok(Outcome::Clients(crate::clients::guidance(
            quality::straining(ctx),
        ))),
        Command::Invite { name, allowance } => invited(ctx, name, allowance).await,
        Command::Reissue { name } => invite::reissued(ctx, name).await.map(Outcome::Invited),
        Command::Remove { name, confirm } => remove::dispatched(ctx, name, confirm).await,
        Command::Outbound => outbound(ctx),
        Command::QualityUpgrade { confirm } => {
            upgrade::upgrade(ctx, confirm).await.map(Outcome::Upgrade)
        }
        Command::Ps { forms } => engine::status(ctx, &forms).await.map(Outcome::Status),
        Command::Doctor {
            narrowing,
            disruptive,
            accept,
        } => diagnosed(ctx, narrowing, disruptive, accept).await,
        Command::Repair {
            consent,
            disruptive,
        } => repair::putting_right(ctx, &consent, disruptive)
            .await
            .map(Outcome::Repair),
        Command::Undo => repair::reversing(ctx).await.map(Outcome::Undo),
        Command::Stored => stored::listing(ctx).map(Outcome::Stored),
        // The one write here, and it is the same answer twice: unconfirmed it lists
        // what would go, confirmed it goes.
        Command::Forget { confirm } => stored::forgetting(ctx, confirm).await.map(Outcome::Stored),
        // The same shape, over the operator's own disk rather than over lemonfiber's
        // files: unconfirmed it accounts and offers, confirmed it takes what the
        // account named as costing nothing.
        Command::Space { confirm } => space::space(ctx, confirm).await.map(Outcome::Space),
        // Held open until the location is lost, which is what a guard is. The
        // interval is this command's own rather than the caller's: a surface that
        // could choose it could choose one that misses the moment it exists for.
        Command::Watch { forms } => watch::supervise(ctx, ctx.volume.as_ref(), &forms, WATCH)
            .await
            .map(Outcome::Watch),
        // Said onto whatever the surface is listening with, which is how a walk is
        // watched rather than read afterwards.
        Command::Walkthrough { item } => walked(ctx, item).await,
        Command::Seed => seed::seed(ctx, false).await.map(Outcome::Seed),
        Command::Adopt => seed::seed(ctx, true).await.map(Outcome::Seed),
        Command::Reset { confirm } => reset::reset(ctx, confirm).await.map(Outcome::Reset),
        Command::Setup(action) => setup::setting_up(ctx, action).await.map(Outcome::Wizard),
        Command::Backup { service } => backup::run(ctx, service).await.map(Outcome::Backup),
        Command::Support {
            write,
            wanted,
            dest,
        } => support::run(ctx, &wanted, write, &dest)
            .await
            .map(Outcome::Support),
        Command::Archives => archives::run(ctx).await.map(Outcome::Archives),
        Command::Restore {
            archive,
            repoint,
            consent,
        } => restore::run(ctx, &archive, repoint, &consent)
            .await
            .map(Outcome::Restore),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::doctor::Narrowing;

    use super::{
        dispatch, pull_progress, Allowance, Command, Ctx, Outcome, QualityAction, SetupAction,
        VersionReport, Waiting,
    };
    use crate::config::Settings;
    use crate::docker::{Condition, State as ServiceState};
    use crate::doctor::Category;
    use crate::model::InvitationStanding;
    use crate::ports::docker::{Engine, Failure as EngineFailure, Health, Lifecycle, LogQuery};
    use crate::ports::process::{Failure, Output, Progress};
    use crate::quality::Preset;
    use crate::stack::Source;
    use crate::test_support::{a_context, nowhere, refused, spoke, Recording, Reporting, Scripted};
    use lemonfiber_fixtures::http::{Answer, Fake};
    use std::time::Duration;

    fn ctx(scripted: Result<Output, Failure>) -> Ctx {
        a_context()
            .runner(Arc::new(Scripted(scripted)))
            .engine(Arc::new(Reporting::default()))
            .build()
    }

    /// A run that keeps archives, whose engine answers and reports nothing running.
    fn keeping_archives(vault: &Arc<crate::app::fixtures::FakeArchive>) -> Ctx {
        let stopped = a_context()
            .engine(Arc::new(lemonfiber_fixtures::support::Reporting::holding(
                &["sonarr"],
                crate::ports::docker::Lifecycle::Exited,
                crate::ports::docker::Health::None,
            )))
            .settings(Settings {
                data_root: Some(std::path::PathBuf::from("/srv/media")),
                ..Settings::default()
            })
            .build();
        crate::app::fixtures::keeping(stopped, vault)
    }

    /// The invitation an answer carries, if it carried one.
    ///
    /// A named reader rather than a `matches!` spanning lines inside an assertion:
    /// that shape leaves the gate a line it cannot see executed, and it reads worse
    /// besides.
    fn invited(made: &Result<Outcome, Box<super::Problem>>) -> Option<&crate::model::Invitation> {
        match made {
            Ok(Outcome::Invited(report)) => Some(report),
            _ => None,
        }
    }

    /// The removal a dispatch answered with, where it answered with one.
    ///
    /// Named for the same reason `invited` is: a `matches!` spanning lines inside an
    /// assertion leaves the gate a line it cannot see executed.
    fn removed(
        said: &Result<Outcome, Box<super::Problem>>,
    ) -> Option<&crate::model::HouseholdRemoval> {
        match said {
            Ok(Outcome::Removed(report)) => Some(report),
            _ => None,
        }
    }

    /// Removing somebody dispatches, and unconfirmed it removes nobody.
    ///
    /// **In-crate as well as out**, because this file is compiled twice — once with this
    /// module and once as the library the `tests/*.rs` binaries link — and a command
    /// dispatched from only one leaves the other copy's arm counted as never run.
    #[tokio::test]
    async fn a_removal_dispatches_and_says_what_it_would_cost() {
        let env = recorded_admin("removing");
        let household = r#"[{"Id":"9","Name":"ana","HasPassword":true,
            "Policy":{"IsAdministrator":false,"EnableAllFolders":true}}]"#;
        let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
        let http = Fake::by_path_in_turn(vec![
            (
                "/Users/AuthenticateByName",
                vec![signed_in.clone(), signed_in],
            ),
            ("/auth/jellyfin", vec![Answer::reply(200, "{}")]),
            (
                "/api/v1/request",
                vec![Answer::reply(
                    200,
                    r#"{"pageInfo":{"results":0},"results":[]}"#,
                )],
            ),
            ("/user/jellyfin/", vec![Answer::reply(404, "")]),
            ("/Users", vec![Answer::reply(200, household)]),
            ("", vec![Answer::reply(200, "[]")]),
        ]);
        let ctx = a_context()
            .settings(Settings {
                env_file: Some(env.clone()),
                ..Settings::default()
            })
            .build()
            .with_http(http);

        let said = dispatch(
            Command::Remove {
                name: "ana".to_owned(),
                confirm: false,
            },
            &ctx,
        )
        .await;

        assert!(
            removed(&said).is_some_and(|report| !report.confirmed && report.name == "ana"),
            "{said:?}"
        );

        // And the other answer the reader has: a refusal is not a removal, which is
        // what stops a test reading one as the other where both are `Ok`-shaped.
        let refused = dispatch(
            Command::Remove {
                name: "  ".to_owned(),
                confirm: false,
            },
            &ctx,
        )
        .await;
        assert!(removed(&refused).is_none(), "{refused:?}");

        // Serialised here as well as from `tests/`: this file is compiled twice, and
        // the envelope arm is a line of the copy that does the serialising — so the
        // copy that never serialises leaves it counted as never run.
        let json = said
            .ok()
            .and_then(|outcome| outcome.envelope().to_json())
            .unwrap_or_default();
        assert!(json.contains(r#""kind":"removal""#), "{json}");
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    /// A scratch environment file holding the media server's recorded password.
    fn recorded_admin(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-invite-{}-{name}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let env = dir.join(".env");
        let _ = crate::config::store::set(
            &env,
            crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
            "minted-earlier",
        );
        env
    }

    /// A reset is driven from this copy of the app layer too.
    ///
    /// **Not a duplicate of the integration test that covers the same command.** The app
    /// layer is compiled twice — once with these tests and once as the library the test
    /// binaries link — and a path driven from only one of them leaves the other counted
    /// as never run. What it asserts is the fact that makes a reset different from an
    /// offer: it answers under its own standing, because nobody is being invited and the
    /// news is that a password they had has stopped working.
    #[tokio::test]
    async fn a_reset_hands_back_an_invitation_under_its_own_standing() {
        let env = recorded_admin("reissues");
        let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
        let http = Fake::by_path_in_turn(vec![
            (
                "/Users/AuthenticateByName",
                vec![signed_in.clone(), signed_in],
            ),
            ("/Users/9/Password", vec![Answer::reply(204, "")]),
            (
                "/Users",
                vec![Answer::reply(
                    200,
                    r#"[{"Id":"9","Name":"ana","HasPassword":true,"Policy":{"IsAdministrator":false}}]"#,
                )],
            ),
        ]);
        let ctx = a_context()
            .settings(Settings {
                env_file: Some(env.clone()),
                household_host: Some("192.168.1.20".to_owned()),
                ..Settings::default()
            })
            .build()
            .with_http(http);

        let said = dispatch(
            Command::Reissue {
                name: "ana".to_owned(),
            },
            &ctx,
        )
        .await;

        assert!(
            invited(&said).is_some_and(|report| report.standing == InvitationStanding::Reset),
            "{said:?}"
        );
        assert!(
            invited(&said).is_some_and(|report| report.hours > 0),
            "a reset carried no window, so nothing will ever act on it: {said:?}"
        );
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    /// Offering an account hands back one address and the name to sign in as.
    ///
    /// Driven through `dispatch` because that is how every surface reaches it: what
    /// comes back is what a browser is handed as well as what the terminal draws.
    #[tokio::test]
    async fn offering_an_account_hands_back_one_address_and_a_name() {
        let env = recorded_admin("offers");
        let http = Fake::by_path_in_turn(vec![
            (
                "/Users/AuthenticateByName",
                vec![
                    Answer::reply(200, r#"{"AccessToken":"token"}"#),
                    Answer::reply(200, r#"{"AccessToken":"token"}"#),
                    Answer::reply(200, r#"{"AccessToken":"token"}"#),
                ],
            ),
            (
                "/System/ActivityLog",
                vec![Answer::reply(200, r#"{"Items":[]}"#)],
            ),
            (
                "/Users/New",
                vec![Answer::reply(
                    200,
                    r#"{"Id":"9","Name":"ana","HasPassword":false}"#,
                )],
            ),
            // The request service answers too, so this copy of the app layer reaches
            // the whole of the linking an invitation does. It is compiled twice, and a
            // path driven from only one of them leaves the other counted as never run.
            ("/auth/jellyfin", vec![Answer::reply(200, "{}")]),
            ("/user/import-from-jellyfin", vec![Answer::reply(201, "{}")]),
            ("/Users", vec![Answer::reply(200, "[]")]),
        ]);
        let ctx = a_context()
            .settings(Settings {
                env_file: Some(env.clone()),
                household_host: Some("192.168.1.20".to_owned()),
                ..Settings::default()
            })
            .build()
            .with_http(http);

        let made = dispatch(
            Command::Invite {
                name: "ana".to_owned(),
                allowance: Allowance::default(),
            },
            &ctx,
        )
        .await;

        let address = invited(&made).map(|report| report.address.clone());

        assert!(
            invited(&made).is_some_and(|report| report.name == "ana"),
            "{made:?}"
        );
        // The address has to be one somebody else can open. Both URLs the stack
        // carries for a service name a host that resolves only on this machine or
        // inside the stack, so "not empty" is satisfied by an address that opens
        // nothing — and the operator would learn it failed from whoever they invited.
        assert!(
            address
                .as_deref()
                .is_some_and(|url| url.contains("192.168.1.20")),
            "the invitation carried an address the household cannot reach: {address:?}"
        );
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    /// A machine nobody can arrive at is said to be one, rather than guessed around.
    ///
    /// An invitation is an address somebody else types. Building one from a default
    /// would send a link that opens nothing, and the operator would learn it had
    /// failed from whoever they invited — so this refuses instead, and says what to
    /// record. Nothing is asked of the media server on the way: the account must not
    /// be made when there is no way to tell anybody about it.
    #[tokio::test]
    async fn a_machine_with_no_address_makes_no_account_and_says_so() {
        let env = recorded_admin("nowhere");
        let http = Fake::silent();
        let recorded = std::sync::Arc::clone(&http);
        let ctx = a_context()
            .settings(Settings {
                env_file: Some(env.clone()),
                ..Settings::default()
            })
            .build()
            .with_http(http);

        let made = dispatch(
            Command::Invite {
                name: "ana".to_owned(),
                allowance: Allowance::default(),
            },
            &ctx,
        )
        .await;

        assert!(
            made.as_ref()
                .err()
                .is_some_and(|problem| problem.code.as_str() == "INVITE-3"),
            "{made:?}"
        );
        let asked = recorded.requests();
        assert!(
            asked.is_empty(),
            "an account was made on a stack with nowhere to send anybody: {asked:?}"
        );
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    /// A media server holding one account, and answering every call this makes.
    ///
    /// `Users` is what the household read returns, and `log` what the record of
    /// account-making returns — the two halves that decide what is already here.
    fn holding(log: &'static str, users: &'static str) -> std::sync::Arc<Fake> {
        let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
        Fake::by_path_in_turn(vec![
            (
                "/Users/AuthenticateByName",
                vec![
                    signed_in.clone(),
                    signed_in.clone(),
                    signed_in.clone(),
                    signed_in,
                ],
            ),
            ("/System/ActivityLog", vec![Answer::reply(200, log)]),
            (
                "/Users/New",
                vec![Answer::reply(
                    200,
                    r#"{"Id":"9","Name":"ana","HasPassword":false}"#,
                )],
            ),
            ("/Users/7", vec![Answer::reply(204, "")]),
            ("/Users", vec![Answer::reply(200, users)]),
        ])
    }

    /// Offer an account on a stack answering with `http`, and hand back both.
    async fn offering(
        env: &std::path::Path,
        http: std::sync::Arc<Fake>,
        name: &str,
    ) -> Result<Outcome, Box<super::Problem>> {
        let ctx = a_context()
            .settings(Settings {
                env_file: Some(env.to_path_buf()),
                household_host: Some("192.168.1.20".to_owned()),
                ..Settings::default()
            })
            .build()
            .with_http(http);
        dispatch(
            Command::Invite {
                name: name.to_owned(),
                allowance: Allowance::default(),
            },
            &ctx,
        )
        .await
    }

    /// An account already here is offered again rather than made a second time.
    ///
    /// The media server refuses a name that differs from one it holds only in case,
    /// and the refusal it gives is `400` — so a match missed here reaches the
    /// operator as the server's own word for a thing they did on purpose. The name
    /// reported is the account's, not the one typed, because that is what somebody
    /// signs in as.
    #[tokio::test]
    async fn an_account_already_here_is_offered_again_rather_than_made_twice() {
        let env = recorded_admin("already");
        let http = holding(
            r#"{"Items":[]}"#,
            r#"[{"Id":"7","Name":"Ana","HasPassword":false}]"#,
        );
        let recorded = std::sync::Arc::clone(&http);

        let made = offering(&env, http, "ana").await;

        assert!(
            invited(&made).is_some_and(
                |report| report.standing == InvitationStanding::Waiting && report.name == "Ana"
            ),
            "{made:?}"
        );
        assert!(
            !recorded.asked_for("/Users/New"),
            "a second account was made for somebody already here"
        );
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    /// Somebody who has already set a password is in the house, not invited.
    #[tokio::test]
    async fn somebody_who_has_already_claimed_an_account_is_reported_as_in() {
        let env = recorded_admin("joined");
        let http = holding(
            r#"{"Items":[]}"#,
            r#"[{"Id":"7","Name":"ana","HasPassword":true}]"#,
        );
        let recorded = std::sync::Arc::clone(&http);

        let made = offering(&env, http, "ana").await;

        assert!(
            invited(&made).is_some_and(|report| report.standing == InvitationStanding::Joined),
            "{made:?}"
        );
        assert!(
            !recorded.asked_for("/Users/New"),
            "a second account was made for somebody already in the house"
        );
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    /// An invitation that has run out is offered again on the account it was for.
    ///
    /// **Not by taking that account back and building another**, which is what this
    /// used to do: the identifier is what everything else in the stack knows somebody
    /// by, so a second account under the same name is the wrong one for anything
    /// holding the first. The window is restarted by dating the invitation again, and
    /// the account it names is untouched.
    #[tokio::test]
    async fn an_invitation_that_ran_out_is_offered_again_on_the_account_it_was_for() {
        let env = recorded_admin("reissue");
        let http = holding(
            r#"{"Items":[{"Type":"UserCreated","Date":"2000-01-01T00:00:00Z","UserId":"7"}]}"#,
            r#"[{"Id":"7","Name":"ana","HasPassword":false}]"#,
        );
        let recorded = std::sync::Arc::clone(&http);

        let made = offering(&env, http, "ana").await;

        assert!(
            invited(&made)
                .is_some_and(|report| report.standing == InvitationStanding::Made
                    && report.withdrawn.is_empty()),
            "{made:?}"
        );
        assert!(
            !recorded.asked_for("/Users/New"),
            "a second account was made under a name the household already holds"
        );
        assert!(
            recorded.asked_for("/Users/7/Password"),
            "the invitation was offered again without being dated again, so the window \
             it promises ran out before it was sent"
        );
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    /// A name that is only spaces is refused here rather than by the media server.
    ///
    /// The server refuses it too, in its own words, which are `400` and a link to
    /// the specification of that status. Nothing is asked of it: the refusal is
    /// about the name, and it is known before anything is opened.
    #[tokio::test]
    async fn an_invitation_for_nobody_is_refused_before_the_server_is_asked() {
        let env = recorded_admin("blank");
        let http = holding(r#"{"Items":[]}"#, "[]");
        let recorded = std::sync::Arc::clone(&http);

        let made = offering(&env, http, "   ").await;

        assert!(
            made.as_ref()
                .err()
                .is_some_and(|problem| problem.code.as_str() == "INVITE-4"),
            "{made:?}"
        );
        let asked = recorded.requests();
        assert!(
            asked.is_empty(),
            "the server was asked about nobody: {asked:?}"
        );
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    /// A name typed with spaces around it is the same person, not a second one.
    ///
    /// The media server keeps the spaces and treats the result as somebody else, so
    /// an untrimmed name makes a second account that reads identically in any list
    /// the two of them appear in.
    #[tokio::test]
    async fn a_name_typed_with_spaces_around_it_is_the_person_of_that_name() {
        let env = recorded_admin("trimmed");
        let http = holding(
            r#"{"Items":[]}"#,
            r#"[{"Id":"7","Name":"ana","HasPassword":false}]"#,
        );
        let recorded = std::sync::Arc::clone(&http);

        let made = offering(&env, http, "  ana  ").await;

        assert!(
            invited(&made).is_some_and(|report| report.standing == InvitationStanding::Waiting),
            "{made:?}"
        );
        assert!(
            !recorded.asked_for("/Users/New"),
            "a second account was made for the same person with spaces round the name"
        );
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    /// An invitation nobody claimed in time is taken back, and named.
    ///
    /// The sweep rides along with the request because nothing runs between commands
    /// to do it on a clock. Asserted by what comes back rather than by the calls
    /// made: an operator who invited somebody last week is owed the sentence saying
    /// the account is gone.
    #[tokio::test]
    async fn an_invitation_nobody_claimed_is_taken_back_and_named() {
        let env = recorded_admin("sweeps");
        let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
        let http = Fake::by_path_in_turn(vec![
            (
                "/Users/AuthenticateByName",
                vec![
                    signed_in.clone(),
                    signed_in.clone(),
                    signed_in.clone(),
                    signed_in,
                ],
            ),
            (
                "/System/ActivityLog",
                vec![Answer::reply(
                    200,
                    r#"{"Items":[{"Type":"UserCreated","Date":"2000-01-01T00:00:00Z","UserId":"7"}]}"#,
                )],
            ),
            (
                "/Users/New",
                vec![Answer::reply(
                    200,
                    r#"{"Id":"9","Name":"ana","HasPassword":false}"#,
                )],
            ),
            ("/Users/7", vec![Answer::reply(204, "")]),
            (
                "/Users",
                vec![Answer::reply(
                    200,
                    r#"[{"Id":"7","Name":"bo","HasPassword":false}]"#,
                )],
            ),
        ]);
        let ctx = a_context()
            .settings(Settings {
                env_file: Some(env.clone()),
                household_host: Some("192.168.1.20".to_owned()),
                ..Settings::default()
            })
            .build()
            .with_http(http);

        let made = dispatch(
            Command::Invite {
                name: "ana".to_owned(),
                allowance: Allowance::default(),
            },
            &ctx,
        )
        .await;

        assert!(
            invited(&made).is_some_and(|report| report.withdrawn == ["bo".to_owned()]),
            "the one nobody claimed was not taken back and named: {made:?}"
        );
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    /// A rehearsal makes no account and takes none back.
    ///
    /// Both halves of this command change the household, and the half that removes
    /// accounts is the one nobody would want rehearsed by doing it. Asserted by what
    /// left the machine rather than by what came back, because an answer that reads
    /// like a rehearsal is exactly what a run that wrote anyway would also print.
    #[tokio::test]
    async fn a_rehearsed_invitation_writes_nothing_to_the_media_server() {
        let env = recorded_admin("rehearsal");
        let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
        let http = Fake::by_path_in_turn(vec![
            (
                "/Users/AuthenticateByName",
                vec![
                    signed_in.clone(),
                    signed_in.clone(),
                    signed_in.clone(),
                    signed_in,
                ],
            ),
            (
                "/System/ActivityLog",
                vec![Answer::reply(
                    200,
                    r#"{"Items":[{"Type":"UserCreated","Date":"2000-01-01T00:00:00Z","UserId":"7"}]}"#,
                )],
            ),
            (
                "/Users/New",
                vec![Answer::reply(
                    200,
                    r#"{"Id":"9","Name":"ana","HasPassword":false}"#,
                )],
            ),
            ("/Users/7", vec![Answer::reply(204, "")]),
            (
                "/Users",
                vec![Answer::reply(
                    200,
                    r#"[{"Id":"7","Name":"bo","HasPassword":false}]"#,
                )],
            ),
        ]);
        let recorded = std::sync::Arc::clone(&http);
        let mut context = a_context()
            .settings(Settings {
                env_file: Some(env.clone()),
                household_host: Some("192.168.1.20".to_owned()),
                ..Settings::default()
            })
            .build();
        context.dry_run = true;
        let ctx = context.with_http(http);

        let made = dispatch(
            Command::Invite {
                name: "ana".to_owned(),
                allowance: Allowance::default(),
            },
            &ctx,
        )
        .await;

        let written: Vec<String> = recorded
            .requests()
            .into_iter()
            .filter(|request| !matches!(request.method, crate::ports::http::Method::Get))
            .map(|request| format!("{:?} {}", request.method, request.url))
            .filter(|line| !line.contains("/Users/AuthenticateByName"))
            .collect();

        assert!(
            written.is_empty(),
            "a rehearsal changed the household: {written:?}"
        );
        assert!(
            invited(&made)
                .is_some_and(|report| report.rehearsed && report.withdrawn == ["bo".to_owned()]),
            "a rehearsal must still say what it would do: {made:?}"
        );
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    /// The record is read from further back than the window it is judged against.
    ///
    /// The server answers with what happened *since* the moment it is given, and the
    /// invitations worth finding are the ones already past their window. Read from the
    /// same moment they are compared to and the answer holds only the ones still
    /// standing, so nothing is ever found to withdraw — a sweep that runs, reports
    /// nothing, and looks exactly like a stack with nothing to sweep.
    ///
    /// The test above cannot catch that: the fake matches on path and hands back its
    /// record whatever moment it is asked for, which a real server would not. So the
    /// moment asked for is the thing to assert, rather than what came back.
    #[tokio::test]
    async fn the_record_is_read_from_further_back_than_the_window_it_judges() {
        let env = recorded_admin("window");
        let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
        let http = Fake::by_path_in_turn(vec![
            (
                "/Users/AuthenticateByName",
                vec![
                    signed_in.clone(),
                    signed_in.clone(),
                    signed_in.clone(),
                    signed_in,
                ],
            ),
            (
                "/System/ActivityLog",
                vec![Answer::reply(200, r#"{"Items":[]}"#)],
            ),
            (
                "/Users/New",
                vec![Answer::reply(
                    200,
                    r#"{"Id":"9","Name":"ana","HasPassword":false}"#,
                )],
            ),
            ("/Users", vec![Answer::reply(200, "[]")]),
        ]);
        let recorded = std::sync::Arc::clone(&http);
        let ctx = a_context()
            .settings(Settings {
                env_file: Some(env.clone()),
                household_host: Some("192.168.1.20".to_owned()),
                ..Settings::default()
            })
            .build()
            .with_http(http);

        let _ = dispatch(
            Command::Invite {
                name: "ana".to_owned(),
                allowance: Allowance::default(),
            },
            &ctx,
        )
        .await;

        let judged = ctx.hours_ago(crate::invitation::HOURS_TO_CLAIM);
        let read_from = recorded
            .requests()
            .into_iter()
            .find(|request| request.url.contains("/System/ActivityLog"))
            .and_then(|request| {
                request
                    .url
                    .split("minDate=")
                    .nth(1)
                    .and_then(|rest| rest.split('&').next())
                    .map(str::to_owned)
            })
            .unwrap_or_default();

        assert!(
            !read_from.is_empty() && read_from < judged,
            "the record was read from {read_from:?}, which is not further back than the \
             {judged} an invitation is judged against — so nothing past its window can be found"
        );
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    /// An invitation serialises under its own kind, for the surface that reads JSON.
    #[tokio::test]
    async fn an_invitation_serialises_under_its_own_kind() {
        let env = recorded_admin("json");
        let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
        let http = Fake::by_path_in_turn(vec![
            (
                "/Users/AuthenticateByName",
                vec![signed_in.clone(), signed_in.clone(), signed_in],
            ),
            (
                "/System/ActivityLog",
                vec![Answer::reply(200, r#"{"Items":[]}"#)],
            ),
            (
                "/Users/New",
                vec![Answer::reply(
                    200,
                    r#"{"Id":"9","Name":"ana","HasPassword":false}"#,
                )],
            ),
            ("/Users", vec![Answer::reply(200, "[]")]),
        ]);
        let ctx = a_context()
            .settings(Settings {
                env_file: Some(env.clone()),
                household_host: Some("192.168.1.20".to_owned()),
                ..Settings::default()
            })
            .build()
            .with_http(http);

        let json = dispatch(
            Command::Invite {
                name: "ana".to_owned(),
                allowance: Allowance::default(),
            },
            &ctx,
        )
        .await
        .ok()
        .and_then(|outcome| outcome.envelope().to_json())
        .unwrap_or_default();

        assert!(json.contains("\"invitation\""), "{json}");
        assert!(json.contains("ana"), "{json}");
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    /// A stack that cannot be read is reported rather than treated as empty.
    #[tokio::test]
    async fn offering_an_account_over_an_unreadable_stack_says_so() {
        let ctx = a_context()
            .over(Source::External(std::path::Path::new(
                "/lemonfiber/no/such/stack",
            )))
            .build()
            .with_http(Fake::silent());

        assert!(
            dispatch(
                Command::Invite {
                    name: "ana".to_owned(),
                    allowance: Allowance::default(),
                },
                &ctx,
            )
            .await
            .is_err(),
            "an unreadable stack was treated as one with no media server"
        );
    }

    /// An invitation the server refuses to take back is not reported as taken back.
    ///
    /// The sweep reports what it did, not what it tried. An operator told an account
    /// was withdrawn would stop expecting that person to appear.
    #[tokio::test]
    async fn an_invitation_the_server_will_not_withdraw_is_not_reported_as_withdrawn() {
        let env = recorded_admin("withdraw-refused");
        let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
        let http = Fake::by_path_in_turn(vec![
            (
                "/Users/AuthenticateByName",
                vec![
                    signed_in.clone(),
                    signed_in.clone(),
                    signed_in.clone(),
                    signed_in,
                ],
            ),
            (
                "/System/ActivityLog",
                vec![Answer::reply(
                    200,
                    r#"{"Items":[{"Type":"UserCreated","Date":"2000-01-01T00:00:00Z","UserId":"7"}]}"#,
                )],
            ),
            (
                "/Users/New",
                vec![Answer::reply(
                    200,
                    r#"{"Id":"9","Name":"ana","HasPassword":false}"#,
                )],
            ),
            ("/Users/7", vec![Answer::reply(500, "no")]),
            (
                "/Users",
                vec![Answer::reply(
                    200,
                    r#"[{"Id":"7","Name":"bo","HasPassword":false}]"#,
                )],
            ),
        ]);
        let ctx = a_context()
            .settings(Settings {
                env_file: Some(env.clone()),
                household_host: Some("192.168.1.20".to_owned()),
                ..Settings::default()
            })
            .build()
            .with_http(http);

        let made = dispatch(
            Command::Invite {
                name: "ana".to_owned(),
                allowance: Allowance::default(),
            },
            &ctx,
        )
        .await;

        assert!(
            invited(&made).is_some_and(|report| report.withdrawn.is_empty()),
            "an account still standing was reported as taken back: {made:?}"
        );
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    /// A media server that refuses the account says so, rather than reporting one.
    ///
    /// The sweep answering and the making failing are different halves: a sweep that
    /// could not run is not a reason to refuse, and a refusal to make the account is.
    #[tokio::test]
    async fn a_media_server_that_refuses_the_account_is_reported() {
        let env = recorded_admin("refuses");
        let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
        let http = Fake::by_path_in_turn(vec![
            (
                "/Users/AuthenticateByName",
                vec![signed_in.clone(), signed_in.clone(), signed_in],
            ),
            (
                "/System/ActivityLog",
                vec![Answer::reply(200, r#"{"Items":[]}"#)],
            ),
            ("/Users/New", vec![Answer::reply(400, "name already taken")]),
            ("/Users", vec![Answer::reply(200, "[]")]),
        ]);
        let ctx = a_context()
            .settings(Settings {
                env_file: Some(env.clone()),
                household_host: Some("192.168.1.20".to_owned()),
                ..Settings::default()
            })
            .build()
            .with_http(http);

        let refused = dispatch(
            Command::Invite {
                name: "ana".to_owned(),
                allowance: Allowance::default(),
            },
            &ctx,
        )
        .await;

        assert!(refused.is_err(), "a refused account was reported as made");
        assert!(
            invited(&refused).is_none(),
            "a refusal carried an invitation anyway"
        );
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    /// A sweep that cannot run does not stop the invitation the operator asked for.
    ///
    /// The sweep is housekeeping riding along; refusing to invite somebody because
    /// last week's invitations could not be counted would be the tail wagging the dog.
    #[tokio::test]
    async fn a_sweep_that_cannot_run_still_makes_the_invitation() {
        let env = recorded_admin("sweep-silent");
        let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
        let http = Fake::by_path_in_turn(vec![
            (
                "/Users/AuthenticateByName",
                vec![signed_in.clone(), signed_in.clone(), signed_in],
            ),
            ("/System/ActivityLog", vec![Answer::Silent]),
            (
                "/Users/New",
                vec![Answer::reply(
                    200,
                    r#"{"Id":"9","Name":"ana","HasPassword":false}"#,
                )],
            ),
            ("/Users", vec![Answer::Silent]),
        ]);
        let ctx = a_context()
            .settings(Settings {
                env_file: Some(env.clone()),
                household_host: Some("192.168.1.20".to_owned()),
                ..Settings::default()
            })
            .build()
            .with_http(http);

        let made = dispatch(
            Command::Invite {
                name: "ana".to_owned(),
                allowance: Allowance::default(),
            },
            &ctx,
        )
        .await;

        assert!(
            invited(&made).is_some_and(|report| report.withdrawn.is_empty()),
            "a sweep that could not run stopped the invitation: {made:?}"
        );
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }

    /// A stack with no media server has nothing to make an account on.
    ///
    /// Built by taking the shipped stack and removing the one service an invitation
    /// needs, so what is under test is the absence rather than a hand-written
    /// manifest that might differ in some other way too.
    #[tokio::test]
    async fn offering_an_account_without_a_media_server_says_there_is_nowhere_to_make_one() {
        static WITHOUT: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
        let dir = WITHOUT.get_or_init(|| {
            let from = std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/media-stack"
            ));
            let to =
                std::env::temp_dir().join(format!("lemonfiber-no-server-{}", std::process::id()));
            let _ = std::fs::create_dir_all(&to);
            let read = std::fs::read_to_string(from.join("stack.toml")).unwrap_or_default();
            // Every block but the media server's, kept in order.
            let kept: String = read
                .split("[[service]]")
                .filter(|block| !block.contains("id = \"jellyfin\""))
                .collect::<Vec<_>>()
                .join("[[service]]");
            let _ = std::fs::write(to.join("stack.toml"), kept);
            to
        });
        let ctx = a_context()
            .over(Source::External(Box::leak(dir.clone().into_boxed_path())))
            .build()
            .with_http(Fake::silent());

        let refused = dispatch(
            Command::Invite {
                name: "ana".to_owned(),
                allowance: Allowance::default(),
            },
            &ctx,
        )
        .await;

        assert!(
            refused.is_err_and(|problem| problem.summary.contains("no media server")),
            "a stack with nothing to make an account on did not say so"
        );
    }

    /// Without a recorded credential there is nothing to ask the server as.
    ///
    /// Refused before anything is attempted rather than after a sign-in fails, so what
    /// comes back names the setup that has not run rather than a rejection.
    #[tokio::test]
    async fn offering_an_account_before_setup_is_refused_rather_than_attempted() {
        let ctx = a_context().build().with_http(Fake::silent());

        let refused = dispatch(
            Command::Invite {
                name: "ana".to_owned(),
                allowance: Allowance::default(),
            },
            &ctx,
        )
        .await;

        assert!(
            refused.is_err(),
            "an invitation was made with no credential to make it with"
        );
        assert!(invited(&refused).is_none(), "a refusal carried one anyway");
    }

    /// What this machine keeps, asked for here as well as from the integration test
    /// beside it — the arms are reached from two compilations of this file and have
    /// to run in both.
    #[tokio::test]
    async fn a_dispatched_disclosure_serialises_under_its_own_kind() {
        let vault = Arc::new(crate::app::fixtures::FakeArchive::roomy());
        let json = dispatch(Command::Stored, &keeping_archives(&vault))
            .await
            .ok()
            .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
            .unwrap_or_default();

        assert!(json.contains("\"kind\":\"stored\""), "{json}");
        assert!(json.contains("\"state\":\"not-asked\""), "{json}");
    }

    /// And the other half of the same answer. An unconfirmed removal lists what
    /// would go and takes none of it, so this reaches the arm without a filesystem
    /// being anywhere near it.
    #[tokio::test]
    async fn an_unconfirmed_removal_answers_with_what_would_go() {
        let vault = Arc::new(crate::app::fixtures::FakeArchive::roomy());
        let json = dispatch(
            Command::Forget { confirm: false },
            &keeping_archives(&vault),
        )
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();

        assert!(json.contains("\"kind\":\"stored\""), "{json}");
        assert!(json.contains("\"state\":\"unconfirmed\""), "{json}");
    }

    #[tokio::test]
    async fn a_dispatched_backup_serialises_under_its_own_kind() {
        let vault = Arc::new(crate::app::fixtures::FakeArchive::roomy());
        let json = dispatch(Command::Backup { service: None }, &keeping_archives(&vault))
            .await
            .ok()
            .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
            .unwrap_or_default();
        assert!(json.contains(r#""kind":"backup""#), "{json}");
        assert!(json.contains("backups"), "it says where it went: {json}");
    }

    #[tokio::test]
    async fn a_dispatched_restore_serialises_under_its_own_kind() {
        // Unconfirmed, so it lists what it would overwrite and touches nothing —
        // which is still the whole of the dispatch, envelope and serialise arms.
        let vault = Arc::new(crate::app::fixtures::FakeArchive::holding(
            crate::app::fixtures::CURRENT,
            crate::backup::SCHEMA,
        ));
        let json = dispatch(
            Command::Restore {
                archive: crate::app::restore::Kept::Named("lemonfiber-full-1.tar.gz".to_owned()),
                repoint: false,
                consent: crate::app::restore::Consent::List,
            },
            &keeping_archives(&vault),
        )
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();
        assert!(json.contains(r#""kind":"restore""#), "{json}");
        assert!(
            json.contains(r#""done":null"#),
            "nothing was put back: {json}"
        );
    }

    #[tokio::test]
    async fn a_dispatched_listing_serialises_under_its_own_kind() {
        // In-crate as well as from `tests/`, because this file carries tests of its
        // own and so is mapped twice: an arm reached only from outside the crate is
        // an arm the mapping this file's own tests build never runs.
        let vault = Arc::new(crate::app::fixtures::FakeArchive::keeping_backups(&[(
            "lemonfiber-full-1.tar.gz",
            "00000000000000000001",
        )]));
        let json = dispatch(Command::Archives, &keeping_archives(&vault))
            .await
            .ok()
            .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
            .unwrap_or_default();
        assert!(json.contains(r#""kind":"archives""#), "{json}");
        assert!(
            json.contains("lemonfiber-full-1.tar.gz"),
            "it names what is kept: {json}"
        );
    }

    #[tokio::test]
    async fn a_dispatched_support_request_serialises_under_its_own_kind() {
        // Told to write nothing, so it describes a bundle and touches no disk —
        // which is still the whole of the dispatch, envelope and serialise arms.
        // In-crate as well as from `tests/`, because this file carries tests of its
        // own and so is mapped twice: an arm reached only from outside the crate is
        // an arm the mapping this file's own tests build never runs.
        let vault = Arc::new(crate::app::fixtures::FakeArchive::roomy());
        let ctx = keeping_archives(&vault).with_http(Fake::silent());
        let json = dispatch(
            Command::Support {
                write: false,
                wanted: super::bundle::Wanted::default(),
                dest: super::support::Destination::Kept,
            },
            &ctx,
        )
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();
        assert!(json.contains(r#""kind":"bundle""#), "{json}");
        assert!(
            json.contains(r#""path":null"#),
            "nothing was written: {json}"
        );
    }

    #[tokio::test]
    async fn a_dispatched_offer_serialises_under_its_own_kind() {
        // In-crate as well as from `tests/`, for the reason the bundle above is:
        // this file carries tests of its own and so is mapped twice, and an arm
        // reached only from outside the crate is one this mapping never runs.
        //
        // Given no consent, so it offers and puts none of it right — which is still
        // the whole of the dispatch, envelope and serialise arms.
        let json = dispatch(
            Command::Repair {
                consent: super::repair::Consent::Offer,
                disruptive: false,
            },
            &crate::app::fixtures::ctx_at("dispatched-offer"),
        )
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();
        assert!(json.contains(r#""kind":"repair""#), "{json}");
        assert!(
            json.contains(r#""acted":false"#),
            "nothing was carried out: {json}"
        );
        // The offer names itself on the way out, which is what a consent sent back
        // in another request has to be able to say.
        assert!(json.contains(r#""agreement":"#), "{json}");
    }

    #[tokio::test]
    async fn a_dispatched_reversal_serialises_under_its_own_kind() {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-dispatched-undo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(dir.join("config"));
        let _ = std::fs::create_dir_all(dir.join("data"));
        let settings = Settings {
            env_file: Some(dir.join("config").join(".env")),
            stack_dir: Some(dir.join("data").join("stack")),
            ..Settings::default()
        };
        let json = dispatch(Command::Undo, &a_context().settings(settings).build())
            .await
            .ok()
            .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
            .unwrap_or_default();
        assert!(json.contains(r#""kind":"undo""#), "{json}");
        assert!(
            json.contains(r#""reversed":[]"#),
            "nothing had been repaired, so nothing went back: {json}"
        );
    }

    #[tokio::test]
    async fn a_dispatched_quality_show_serialises_under_its_own_kind() {
        // Through dispatch, a quality command reaches its outcome, envelope and
        // serialisation — the arms the handler's own tests, calling it directly,
        // never touch. With no config the choice is the default, shown.
        let json = dispatch(Command::Quality(QualityAction::Show), &ctx(Ok(spoke(""))))
            .await
            .ok()
            .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
            .unwrap_or_default();
        assert!(
            json.contains("\"kind\":\"quality\""),
            "envelope names the kind"
        );
        assert!(json.contains("everything"), "the global choice is reported");
    }

    #[tokio::test]
    async fn a_dispatched_quality_set_with_nowhere_to_record_is_an_error() {
        // The dispatch arm unboxes the handler's error: a set with no configured
        // env file has nowhere to write the choice, so it fails rather than lying.
        let refused = dispatch(
            Command::Quality(QualityAction::Set {
                preset: Preset::Balanced,
                media_type: None,
                confirm: false,
            }),
            &ctx(Ok(spoke(""))),
        )
        .await;
        assert!(
            refused.is_err(),
            "a set with nowhere to record cannot succeed"
        );
    }

    #[tokio::test]
    async fn a_dispatched_trace_serialises_under_its_own_kind() {
        // No key opens a target, so no item matches and the trace stays offline while
        // exercising the dispatch, envelope and serialise arms for its outcome.
        let json = dispatch(
            Command::Trace {
                term: "the expanse".to_owned(),
                season: None,
                searching: false,
            },
            &ctx(Ok(spoke(""))),
        )
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();
        assert!(
            json.contains("\"kind\":\"trace\""),
            "envelope names the kind"
        );
    }

    #[tokio::test]
    async fn a_dispatched_household_serialises_under_its_own_kind() {
        // Nothing is recorded to sign in with, so the view reports itself unavailable —
        // which still exercises the dispatch, envelope and serialise arms for its outcome.
        let json = dispatch(Command::Household { member: None }, &ctx(Ok(spoke(""))))
            .await
            .ok()
            .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
            .unwrap_or_default();
        assert!(
            json.contains("\"kind\":\"household\""),
            "envelope names the kind"
        );
    }

    #[tokio::test]
    async fn a_dispatched_front_door_serialises_under_its_own_kind() {
        // The stack this repository carries runs a request service, so the answer
        // names one — and the command runs through dispatch, envelope and serialise
        // for its outcome on the way.
        let household = a_context()
            .engine(Arc::new(Reporting::holding(
                &["seerr"],
                crate::ports::docker::Lifecycle::Running,
                crate::ports::docker::Health::Healthy,
            )))
            .build();
        let json = dispatch(Command::FrontDoor, &household)
            .await
            .ok()
            .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
            .unwrap_or_default();
        assert!(
            json.contains("\"kind\":\"front-door\""),
            "envelope names the kind"
        );
    }

    #[tokio::test]
    async fn a_dispatched_stuck_serialises_under_its_own_kind() {
        // No key opens a target, so nothing is stuck — but the command still runs through
        // dispatch, envelope and serialise for its outcome.
        let json = dispatch(Command::Stuck, &ctx(Ok(spoke(""))))
            .await
            .ok()
            .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
            .unwrap_or_default();
        assert!(
            json.contains("\"kind\":\"stuck\""),
            "envelope names the kind"
        );
    }

    /// The whole of what leaves this machine, asked for here as well as from the
    /// integration test beside it — the arm is reached from two compilations of this
    /// file and has to run in both.
    #[tokio::test]
    async fn a_dispatched_enumeration_serialises_under_its_own_kind() {
        let json = dispatch(Command::Outbound, &ctx(Ok(spoke(""))))
            .await
            .ok()
            .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
            .unwrap_or_default();

        assert!(json.contains("\"kind\":\"outbound\""), "{json}");
        assert!(json.contains("\"reach\":\"registry\""), "{json}");
    }

    /// And the refusal, because half an enumeration reads as the whole of it: a stack
    /// that will not read cannot say what its services reach.
    #[tokio::test]
    async fn an_enumeration_over_a_stack_that_will_not_read_is_refused() {
        let nowhere = a_context()
            .over(crate::test_support::nowhere())
            .runner(Arc::new(Scripted(Ok(spoke("")))))
            .engine(Arc::new(Reporting::default()))
            .build();

        assert!(dispatch(Command::Outbound, &nowhere).await.is_err());
    }

    /// The words need no stack and no engine, so this is the one command that runs
    /// through dispatch, envelope and serialise against a context that has nothing.
    #[tokio::test]
    async fn a_dispatched_explanation_serialises_under_its_own_kind() {
        let ctx = ctx(Ok(spoke("")));

        let word = dispatch(
            Command::Explain {
                word: "indexer".to_owned(),
            },
            &ctx,
        )
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();
        assert!(word.contains("\"kind\":\"word\""), "{word}");
        assert!(word.contains("\"word\":\"indexer\""), "{word}");

        let listed = dispatch(Command::Glossary, &ctx)
            .await
            .ok()
            .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
            .unwrap_or_default();
        assert!(listed.contains("\"kind\":\"glossary\""), "{listed}");
        assert!(listed.contains("\"words\":[{"), "{listed}");
    }

    /// Which app to watch on needs no stack and no engine either, for the same
    /// reason: the client landscape belongs to the platforms rather than to this
    /// machine, so it answers before anything is set up.
    #[tokio::test]
    async fn which_app_to_watch_on_answers_against_a_context_that_has_nothing() {
        let ctx = ctx(Ok(spoke("")));

        let said = dispatch(Command::Clients, &ctx)
            .await
            .ok()
            .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
            .unwrap_or_default();

        assert!(said.contains("\"kind\":\"clients\""), "{said}");
        assert!(said.contains("\"devices\":[{"), "{said}");
        assert!(said.contains("\"only_at_home\""), "{said}");
    }

    /// Answering "it means nothing" for a word this product never explains would be a
    /// wrong answer where an absent one was wanted.
    #[tokio::test]
    async fn a_word_with_no_entry_is_refused_rather_than_answered_emptily() {
        let refused = dispatch(
            Command::Explain {
                word: "kubernetes".to_owned(),
            },
            &ctx(Ok(spoke(""))),
        )
        .await
        .err()
        .map(|problem| problem.code.as_str().to_owned());

        assert_eq!(refused.as_deref(), Some("WORD-1"));
    }

    #[tokio::test]
    async fn a_dispatched_setup_serialises_under_its_own_kind() {
        // Reading where the walk stands asks nothing and writes nothing, so it runs
        // through dispatch, envelope and serialise on a machine with no setup at all.
        let env_file = config_scratch("setup-kind");
        let settings = Settings {
            stack_dir: env_file.parent().map(|dir| dir.join("stack")),
            env_file: Some(env_file),
            ..Settings::default()
        };
        let json = dispatch(
            Command::Setup(SetupAction::Where),
            &a_context().settings(settings).build(),
        )
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();
        assert!(
            json.contains("\"kind\":\"wizard\""),
            "envelope names the kind: {json}"
        );
        assert!(json.contains("\"at\":\"welcome\""), "{json}");
    }

    #[tokio::test]
    async fn a_dispatched_reset_serialises_under_its_own_kind() {
        // The test stack is external, so a reset reverts nothing — but the command still
        // runs through dispatch, envelope and serialise for its outcome.
        let json = dispatch(Command::Reset { confirm: false }, &ctx(Ok(spoke(""))))
            .await
            .ok()
            .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
            .unwrap_or_default();
        assert!(
            json.contains("\"kind\":\"reset\""),
            "envelope names the kind"
        );
    }

    #[tokio::test]
    async fn a_dispatched_music_choice_serialises_under_its_own_kind() {
        // A rehearsal records nothing and reaches no service, so it stays offline while
        // exercising the dispatch, envelope and serialise arms for its outcome.
        let mut context = ctx(Ok(spoke("")));
        context.dry_run = true;
        let json = dispatch(
            Command::QualityMusic {
                format: crate::audio::Format::Lossless,
            },
            &context,
        )
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();
        assert!(
            json.contains("\"kind\":\"music\""),
            "envelope names the kind"
        );
    }

    #[tokio::test]
    async fn a_dispatched_quality_upgrade_serialises_under_its_own_kind() {
        // Unconfirmed, it states the cost and reaches no service, so it stays offline
        // while exercising the dispatch, envelope and serialise arms for its outcome.
        let json = dispatch(
            Command::QualityUpgrade { confirm: false },
            &ctx(Ok(spoke(""))),
        )
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();
        assert!(
            json.contains("\"kind\":\"upgrade\""),
            "envelope names the kind"
        );
    }

    #[tokio::test]
    async fn a_dispatched_accounting_of_the_disk_serialises_under_its_own_kind() {
        // A machine with no data location has nothing to account for, which is the
        // one answer this reaches with nothing running and nothing on a disk — and
        // it exercises the dispatch arm, which is what this is here for.
        let refused = dispatch(Command::Space { confirm: false }, &ctx(Ok(spoke(""))))
            .await
            .err()
            .map(|problem| problem.code);
        assert_eq!(refused, Some(crate::space::NOWHERE_TO_MEASURE));
    }

    #[tokio::test]
    async fn a_dispatched_upgrade_over_an_unreadable_stack_is_an_error() {
        // The dispatch arm unboxes the driver's error: a confirmed upgrade cannot read
        // an unreadable stack's services, so it fails rather than half-acting.
        let ctx = a_context()
            .engine(Arc::new(Reporting::default()))
            .over(nowhere())
            .build();
        assert!(dispatch(Command::QualityUpgrade { confirm: true }, &ctx)
            .await
            .is_err());
    }

    /// What a version report looks like when the engine said `compose`.
    ///
    /// Tests assert against the whole outcome rather than picking it apart. A
    /// destructuring assertion needs a branch for the case that cannot happen,
    /// and that branch is a line no test can ever cover.
    fn reported(compose: Option<&str>) -> Outcome {
        Outcome::Version(VersionReport {
            binary: env!("CARGO_PKG_VERSION").to_owned(),
            supported_schema: vec![1],
            stack: "0.1.0".to_owned(),
            compose: compose.map(str::to_owned),
        })
    }

    /// Forms come from the stack rather than from lemonfiber, so this reports what the
    /// manifest declares — including whether each one may be combined, which is what an
    /// operator choosing between two of them needs to know before they try.
    #[tokio::test]
    async fn lists_the_forms_the_stack_declares_in_its_own_words() {
        let ctx = ctx(Ok(spoke("v2.32.1\n")));
        let listed = dispatch(Command::Forms, &ctx).await;

        assert!(matches!(&listed, Ok(Outcome::Forms(report))
            if report.forms.len() > 1
                && report
                    .forms
                    .iter()
                    .any(|form| form.id == "search" && form.name == "Search" && form.composable)));
    }

    /// Also driven from `tests/forms.rs`, against the real stack. Kept here as well
    /// because this crate is compiled twice — once with its own test modules and once as
    /// the library those binaries link — and a command dispatched from only one of them
    /// leaves the other's copy of the arm counted as never run.
    #[tokio::test]
    async fn a_preview_is_dispatched_like_any_other_command() {
        let ctx = ctx(Ok(spoke("v2.32.1\n")));
        let previewed = dispatch(
            Command::Preview {
                forms: vec!["library".to_owned()],
            },
            &ctx,
        )
        .await;

        assert!(
            matches!(&previewed, Ok(Outcome::Preview(plan))
                if plan.services.contains(&"jellyfin".to_owned())),
            "{previewed:?}"
        );
        assert_eq!(
            previewed.ok().map(|outcome| outcome.envelope().kind),
            Some(crate::model::kind::PREVIEW),
            "the kind names the question that was asked"
        );
    }

    #[tokio::test]
    async fn reports_the_engine_version_when_the_engine_answers() {
        let ctx = ctx(Ok(spoke("v2.32.1\n")));
        assert_eq!(
            dispatch(Command::Version, &ctx).await,
            Ok(reported(Some("v2.32.1")))
        );
    }

    #[tokio::test]
    async fn still_answers_when_the_engine_is_missing() {
        let ctx = ctx(Err(Failure::NotFound {
            program: "docker".to_owned(),
        }));
        assert_eq!(dispatch(Command::Version, &ctx).await, Ok(reported(None)));
    }

    #[tokio::test]
    async fn treats_an_engine_that_fails_as_one_that_did_not_answer() {
        let ctx = ctx(Ok(refused("permission denied")));
        assert_eq!(dispatch(Command::Version, &ctx).await, Ok(reported(None)));
    }

    #[tokio::test]
    async fn an_unusable_engine_is_also_reported_as_absent() {
        let ctx = ctx(Err(Failure::Unusable {
            program: "docker".to_owned(),
            reason: "denied".to_owned(),
        }));
        assert_eq!(dispatch(Command::Version, &ctx).await, Ok(reported(None)));
    }

    #[tokio::test]
    async fn an_outcome_serialises_inside_the_versioned_envelope() {
        let ctx = ctx(Ok(spoke("v2.32.1")));
        let rendered = dispatch(Command::Version, &ctx)
            .await
            .ok()
            .and_then(|outcome| outcome.envelope().to_json());
        assert_eq!(
            rendered.as_deref(),
            Some(concat!(
                r#"{"api_version":1,"kind":"version","data":{"binary":""#,
                env!("CARGO_PKG_VERSION"),
                r#"","supported_schema":[1],"stack":"0.1.0","compose":"v2.32.1"}}"#
            ))
        );
    }

    #[tokio::test]
    async fn a_stack_that_cannot_be_read_is_reported_rather_than_left_out() {
        let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
        let ctx = a_context()
            .runner(Arc::new(Scripted(Ok(spoke("v2.32.1")))))
            .engine(Arc::new(Reporting::default()))
            .over(nowhere)
            .build();
        let refusal = dispatch(Command::Version, &ctx)
            .await
            .err()
            .map(|problem| problem.code);
        assert_eq!(
            refusal,
            Some(crate::stack::STACK_UNREADABLE),
            "an operator's own --stack-dir mistake reaches them"
        );
    }

    /// A context that runs against the checked-out stack, in rehearsal.
    fn rehearsing(protocols: crate::config::Protocols) -> Ctx {
        let settings = Settings {
            protocols,
            ..Settings::default()
        };
        a_context()
            .engine(Arc::new(Reporting::default()))
            .settings(settings)
            .build()
            .rehearsing()
    }

    fn report(
        outcome: Result<Outcome, Box<super::Problem>>,
    ) -> Option<crate::model::LifecycleReport> {
        match outcome {
            Ok(Outcome::Lifecycle(report)) => Some(report),
            Ok(
                Outcome::Version(_)
                | Outcome::Forms(_)
                | Outcome::Preview(_)
                | Outcome::Config(_)
                | Outcome::Quality(_)
                | Outcome::Upgrade(_)
                | Outcome::Music(_)
                | Outcome::Trace(_)
                | Outcome::Household(_)
                | Outcome::FrontDoor(_)
                | Outcome::Stuck(_)
                | Outcome::Word(_)
                | Outcome::Glossary(_)
                | Outcome::Clients(_)
                | Outcome::Invited(_)
                | Outcome::Removed(_)
                | Outcome::Outbound(_)
                | Outcome::Stored(_)
                | Outcome::Space(_)
                | Outcome::Status(_)
                | Outcome::Doctor(_)
                | Outcome::Repair(_)
                | Outcome::Undo(_)
                | Outcome::Seed(_)
                | Outcome::Reset(_)
                | Outcome::Wizard(_)
                | Outcome::Backup(_)
                | Outcome::Support(_)
                | Outcome::Archives(_)
                | Outcome::Restore(_)
                | Outcome::Watch(_)
                | Outcome::Walkthrough(_),
            )
            | Err(_) => None,
        }
    }

    fn diagnosis(
        outcome: Result<Outcome, Box<super::Problem>>,
    ) -> Option<crate::model::DoctorReport> {
        match outcome {
            Ok(Outcome::Doctor(report)) => Some(report),
            Ok(
                Outcome::Version(_)
                | Outcome::Forms(_)
                | Outcome::Preview(_)
                | Outcome::Lifecycle(_)
                | Outcome::Config(_)
                | Outcome::Quality(_)
                | Outcome::Upgrade(_)
                | Outcome::Music(_)
                | Outcome::Trace(_)
                | Outcome::Household(_)
                | Outcome::FrontDoor(_)
                | Outcome::Stuck(_)
                | Outcome::Word(_)
                | Outcome::Glossary(_)
                | Outcome::Clients(_)
                | Outcome::Invited(_)
                | Outcome::Removed(_)
                | Outcome::Outbound(_)
                | Outcome::Stored(_)
                | Outcome::Space(_)
                | Outcome::Status(_)
                | Outcome::Repair(_)
                | Outcome::Undo(_)
                | Outcome::Seed(_)
                | Outcome::Reset(_)
                | Outcome::Wizard(_)
                | Outcome::Backup(_)
                | Outcome::Support(_)
                | Outcome::Archives(_)
                | Outcome::Restore(_)
                | Outcome::Watch(_)
                | Outcome::Walkthrough(_),
            )
            | Err(_) => None,
        }
    }

    /// Asked for twice, an operation does the same thing twice rather than refusing the
    /// second time. The stack is the state, not a record of what has been asked for, so
    /// an operator who is unsure whether a command landed can simply run it again.
    #[tokio::test]
    async fn asking_twice_is_not_an_error_the_second_time() {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        let ctx = a_context()
            .runner(Arc::new(Scripted(Ok(spoke("")))))
            .engine(Arc::new(Reporting::holding(
                &LIBRARY,
                Lifecycle::Running,
                Health::Healthy,
            )))
            .settings(settings)
            .build()
            .with_http(Fake::scripted(Vec::new()));

        for command in [
            Command::Up {
                forms: vec!["library".to_owned()],
            },
            Command::Down {
                forms: vec!["library".to_owned()],
                wait: Waiting::Never,
            },
            Command::Switch {
                forms: vec!["library".to_owned()],
            },
        ] {
            let once = report(dispatch(command.clone(), &ctx).await)
                .map(|report| (report.action, report.status));
            let again = report(dispatch(command.clone(), &ctx).await)
                .map(|report| (report.action, report.status));

            assert_eq!(
                once, again,
                "the second {command:?} answers as the first did"
            );
            assert_eq!(
                once.as_ref().map(|(_, status)| *status),
                Some(Some(0)),
                "and neither is a refusal: {once:?}"
            );
        }
    }

    /// The environment checks are about the machine rather than about anything
    /// running on it, so none of their findings names a service — and a run with
    /// nothing to quote asks the engine for nothing at all.
    #[tokio::test]
    async fn a_run_with_no_service_in_trouble_quotes_nothing() {
        let report = diagnosis(
            dispatch(
                Command::Doctor {
                    narrowing: Narrowing::Category(Category::Environment),
                    disruptive: false,
                    accept: None,
                },
                &watching(Reporting::holding(
                    &LIBRARY,
                    Lifecycle::Running,
                    Health::Healthy,
                )),
            )
            .await,
        );

        assert!(
            report
                .as_ref()
                .is_some_and(|report| !report.findings.is_empty()),
            "the environment checks did run: {report:?}"
        );
        assert!(
            report
                .iter()
                .flat_map(|report| report.findings.iter())
                .all(|finding| finding.said.is_none()),
            "nothing here is about a service, so nothing has a service to quote"
        );
    }

    /// A check can say a service is not answering; only the service can say why. So
    /// what it said lately travels with the finding rather than waiting for the
    /// operator to go and fetch it.
    ///
    /// Driven against the decision itself rather than through a whole run, because
    /// which checks name a service is a separate question from what happens to a
    /// finding that does — today only the credential check names one, and this has to
    /// keep working as more of them do.
    #[tokio::test]
    async fn a_failing_finding_carries_what_its_service_said() {
        let ctx = watching(
            Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
                .saying("jellyfin", "auth failed: bad credentials"),
        );

        let quoted = super::engine::quoted(
            &ctx,
            vec![crate::doctor::Finding::in_category(
                Category::Services,
                "services.jellyfin",
                "Jellyfin answers",
                crate::doctor::Verdict::Fail(crate::error::Problem::new(
                    crate::error::Code::new("TEST-1"),
                    crate::error::Severity::Error,
                    "it is not answering",
                    "it means what it says",
                    crate::error::Remedy::new("put it right"),
                )),
            )
            .about("jellyfin")],
        )
        .await;

        assert_eq!(
            quoted.first().and_then(|finding| finding.said.clone()),
            Some("auth failed: bad credentials\n".to_owned()),
            "the service's own words, unprefixed — the finding already names it"
        );
    }

    /// A stand-in for a credential a service quotes back at itself, assembled rather
    /// than written out so no value that reads as one sits in this source.
    fn a_credential() -> String {
        ["abcdef", "1234", "567890"].concat()
    }

    /// What a service said is somebody else's text, and a service that fails while
    /// authenticating says so with the credential in hand.
    ///
    /// Withheld where the field is built rather than where it is drawn, which is what
    /// this asserts: `said` is served on `/api/checks` as well as printed, so a report
    /// that looked clean would still have been publishing the key.
    #[tokio::test]
    async fn what_a_service_said_reaches_a_finding_with_no_credential_in_it() {
        let secret = a_credential();
        let ctx = watching(
            Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
                .saying(
                    "jellyfin",
                    &format!("startup: api_key={secret} was rejected"),
                )
                .saying("jellyfin", &format!("INDEXER_APIKEY={secret}")),
        );

        let said = what_it_said(&ctx).await;
        assert!(!said.contains(&secret), "{said}");
        // The sentence around it, still whole. A redactor that took the credential by
        // taking the line with it would leave a finding with no evidence under it, which
        // is the failure this path is here to prevent.
        assert!(said.contains("startup:"), "{said}");
        assert!(said.contains("was rejected"), "{said}");
        assert!(said.contains("INDEXER_APIKEY"), "{said}");
    }

    /// A container quoting an outbound URL back at itself, with the key not the first
    /// parameter in it.
    ///
    /// The shape this product builds its own indexer request in — `t=search` first and
    /// the key last — and the \*arrs log an outbound URL whenever one is refused. What a
    /// container writes is the input this field is made of, so a rule that reads a URL
    /// only as far as its first `=` leaves the key in the evidence.
    #[tokio::test]
    async fn a_url_a_container_quotes_back_arrives_with_no_key_in_it() {
        let secret = a_credential();
        let ctx = watching(
            Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy).saying(
                "jellyfin",
                &format!("GET https://indexer.example/api?t=search&apikey={secret} returned 401"),
            ),
        );

        let said = what_it_said(&ctx).await;
        assert!(!said.contains(&secret), "{said}");
        assert!(said.contains("https://indexer.example/api"), "{said}");
        assert!(said.contains("returned 401"), "{said}");
    }

    /// A log line opening on one word, which is how most of them open.
    ///
    /// `ERROR:`, `WARN:`, `Unauthorized:` — a single word and a colon, which is the exact
    /// shape of a setting whose value follows it. Every word this reads as a marker is
    /// ordinary English, so a line opening on one loses the sentence it introduced, and
    /// the sentence is the whole of what the finding was gathering.
    #[tokio::test]
    async fn a_line_opening_on_one_word_keeps_the_sentence_after_it() {
        let refused = "the request was refused by the indexer";
        let ctx = watching(
            Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
                .saying("jellyfin", &format!("Unauthorized: {refused}")),
        );

        let said = what_it_said(&ctx).await;
        assert!(said.contains(refused), "{said}");
    }

    /// What one troubled service's finding carries, for the tests that are about the
    /// text rather than about which findings get one.
    async fn what_it_said(ctx: &Ctx) -> String {
        super::engine::quoted(
            ctx,
            vec![crate::doctor::Finding::in_category(
                Category::Services,
                "services.jellyfin",
                "Jellyfin answers",
                crate::doctor::Verdict::Fail(crate::error::Problem::new(
                    crate::error::Code::new("TEST-1"),
                    crate::error::Severity::Error,
                    "it is not answering",
                    "it means what it says",
                    crate::error::Remedy::new("put it right"),
                )),
            )
            .about("jellyfin")],
        )
        .await
        .first()
        .and_then(|finding| finding.said.clone())
        .unwrap_or_default()
    }

    /// Evidence for something that works is noise, and on a healthy run it would be
    /// the bulk of the output.
    #[tokio::test]
    async fn a_finding_that_passed_carries_nothing() {
        let ctx = watching(
            Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
                .saying("jellyfin", "started"),
        );

        let quoted = super::engine::quoted(
            &ctx,
            vec![crate::doctor::Finding::in_category(
                Category::Services,
                "services.jellyfin",
                "Jellyfin answers",
                crate::doctor::Verdict::Pass { note: None },
            )
            .about("jellyfin")],
        )
        .await;

        assert!(quoted.first().is_some_and(|finding| finding.said.is_none()));
    }

    #[tokio::test]
    async fn doctor_runs_the_checks_and_reports_them_in_the_envelope() {
        // The engine here does not host the torrent pair, so the findings are
        // not green — but dispatch's job is only to run the checks and hand back
        // what they found, named in the machine-readable envelope.
        let ctx = watching(Reporting::holding(
            &LIBRARY,
            Lifecycle::Running,
            Health::Healthy,
        ));
        let command = Command::Doctor {
            narrowing: Narrowing::Category(Category::Vpn),
            disruptive: false,
            accept: None,
        };
        let outcome = dispatch(command, &ctx).await;

        let json = outcome
            .as_ref()
            .ok()
            .and_then(|outcome| outcome.clone().envelope().to_json());
        assert!(
            json.as_deref()
                .is_some_and(|json| json.contains(r#""kind":"doctor""#)
                    && json.contains(r#""category":"vpn""#)),
            "the doctor envelope should name itself and carry vpn findings: {json:?}"
        );

        let report = diagnosis(outcome);
        assert!(report.is_some_and(|report| !report.findings.is_empty()
            && report
                .findings
                .iter()
                .all(|finding| finding.category == Category::Vpn)));
    }

    #[tokio::test]
    async fn a_full_doctor_run_includes_the_quality_guide_check() {
        // The guide-source check is wired into the suite: an unfiltered run carries
        // its finding. The ctx's offline http makes it unverified rather than
        // reaching the real upstream.
        let ctx = watching(Reporting::holding(
            &LIBRARY,
            Lifecycle::Running,
            Health::Healthy,
        ));
        let outcome = dispatch(
            Command::Doctor {
                narrowing: Narrowing::Suite,
                disruptive: false,
                accept: None,
            },
            &ctx,
        )
        .await;

        let names = diagnosis(outcome)
            .map(|report| {
                report
                    .findings
                    .into_iter()
                    .map(|finding| finding.check)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        assert!(
            names.iter().any(|check| check == "services.quality-guides"),
            "the guide-source check should appear in a full run: {names:?}"
        );
    }

    /// One check, named the way the report names it.
    ///
    /// The whole point of the identifier being the same on both sides: the guide check
    /// shares its family with the release search, so a family cannot single it out.
    #[tokio::test]
    async fn naming_one_check_runs_that_check_alone() {
        let ctx = watching(Reporting::holding(
            &LIBRARY,
            Lifecycle::Running,
            Health::Healthy,
        ));
        let outcome = dispatch(
            Command::Doctor {
                narrowing: Narrowing::Check("services.quality-guides".to_owned()),
                disruptive: false,
                accept: None,
            },
            &ctx,
        )
        .await;

        let names = diagnosis(outcome)
            .map(|report| {
                report
                    .findings
                    .into_iter()
                    .map(|finding| finding.check)
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();
        assert_eq!(
            names,
            vec!["services.quality-guides".to_owned()],
            "the run should hold the named check and nothing beside it"
        );
    }

    /// A name nothing reports is refused rather than answered with an empty report,
    /// which reads as a stack with nothing wrong with it.
    #[tokio::test]
    async fn a_check_this_stack_does_not_report_is_refused() {
        let ctx = watching(Reporting::holding(
            &LIBRARY,
            Lifecycle::Running,
            Health::Healthy,
        ));
        let outcome = dispatch(
            Command::Doctor {
                narrowing: Narrowing::Check("services.nothing-of-the-kind".to_owned()),
                disruptive: false,
                accept: None,
            },
            &ctx,
        )
        .await;

        assert_eq!(
            outcome.as_ref().err().map(|problem| problem.code),
            Some(crate::error::Code::new("DIAG-1")),
            "a name this stack does not report should be refused: {outcome:?}"
        );
    }

    /// The two budget constants are bounded beside their own definition. This is about
    /// the suite that actually gets built: a check overriding `budget()` with a minute
    /// of its own would break the promise without touching either constant.
    ///
    /// The slowest check is the whole of what has to fit, because the checks run
    /// concurrently — a run costs its slowest rather than their sum, which is the only
    /// reason a filesystem may ask for twice what a container command gets.
    #[tokio::test]
    async fn no_check_in_a_non_disruptive_run_may_outlast_the_run_itself() {
        let ctx = a_context()
            .engine(Arc::new(Reporting::holding(
                &[],
                Lifecycle::Exited,
                Health::None,
            )))
            .build();

        let slowest = super::engine::assembled(&ctx, false)
            .await
            .ok()
            .and_then(|(_, checks)| checks.iter().map(|check| check.budget()).max());

        assert!(
            slowest.is_some_and(|budget| budget <= Duration::from_secs(30)),
            "a full non-disruptive run is meant to finish inside thirty seconds: {slowest:?}"
        );
    }

    #[tokio::test]
    async fn doctor_reports_an_unreadable_stack_rather_than_guessing() {
        let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
        let ctx = a_context()
            .runner(Arc::new(Scripted(Ok(spoke("v2.32.1")))))
            .engine(Arc::new(Reporting::default()))
            .over(nowhere)
            .build();
        let outcome = dispatch(
            Command::Doctor {
                narrowing: Narrowing::Suite,
                disruptive: false,
                accept: None,
            },
            &ctx,
        )
        .await;
        assert_eq!(
            outcome.as_ref().err().map(|problem| problem.code),
            Some(crate::stack::STACK_UNREADABLE)
        );
        assert!(diagnosis(outcome).is_none());
    }

    #[tokio::test]
    async fn starting_a_form_reports_what_it_would_run_and_runs_nothing() {
        let ctx = rehearsing(crate::config::Protocols::both());
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };
        let produced = report(dispatch(command, &ctx).await);
        assert_eq!(
            produced.map(|report| (
                report.action,
                report.plan.profiles.into_iter().collect::<Vec<String>>(),
                report.rehearsed,
                report.status,
                report.command.last().cloned()
            )),
            Some((
                "up".to_owned(),
                vec!["media".to_owned()],
                true,
                None,
                Some("--detach".to_owned())
            )),
            "a rehearsal reports the command and never ran it"
        );
    }

    #[tokio::test]
    async fn what_the_configuration_left_out_is_reported_rather_than_dropped() {
        let ctx = rehearsing(crate::config::Protocols {
            usenet: true,
            torrent: false,
        });
        let command = Command::Up {
            forms: vec!["tv".to_owned()],
        };
        let produced = report(dispatch(command, &ctx).await);

        assert_eq!(
            produced.map(|report| report.plan.dropped),
            Some(vec![crate::stack::closure::Dropped {
                profile: "torrent".to_owned(),
                needs: lemonfiber_manifest::Protocol::Torrent,
            }]),
            "the operator hears which service is missing, and why"
        );
    }

    #[tokio::test]
    async fn a_real_run_reports_how_the_command_exited() {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        // Reachable and holding nothing: stopping asks what else is running before it
        // acts, and an engine that will not answer is a refusal rather than a run.
        let ctx = a_context()
            .engine(Arc::new(Reporting::holding(
                &[],
                Lifecycle::Exited,
                Health::None,
            )))
            .settings(settings)
            .build();
        let command = Command::Down {
            forms: vec!["library".to_owned()],
            wait: Waiting::Never,
        };
        let produced = report(dispatch(command, &ctx).await);

        assert_eq!(
            produced.map(|report| (report.action, report.rehearsed, report.status)),
            Some(("down".to_owned(), false, Some(0)))
        );
    }

    #[tokio::test]
    async fn starting_named_services_is_the_start_compose_spells_that_way() {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        let ctx = a_context()
            .engine(Arc::new(Reporting::holding(
                &["sabnzbd", "gluetun", "qbittorrent"],
                Lifecycle::Running,
                Health::Healthy,
            )))
            .settings(settings)
            .build()
            .waiting(std::time::Duration::ZERO);
        let command = Command::Start {
            forms: vec!["dl".to_owned()],
            services: vec!["qbittorrent".to_owned()],
        };
        let produced = report(dispatch(command, &ctx).await);

        assert_eq!(
            produced
                .as_ref()
                .map(|report| report.action.clone())
                .as_deref(),
            Some("up"),
            "it is a start, and reports as one"
        );
        assert!(
            produced.is_some_and(|report| report.command.contains(&"qbittorrent".to_owned())),
            "and the command it ran names the service rather than the form"
        );
    }

    #[tokio::test]
    async fn a_teardown_asked_to_wait_where_nothing_is_downloading_stops_at_once() {
        // A form holding no download client asks the network nothing, so the wait
        // it was asked for is over before the teardown that follows it begins.
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        let ctx = a_context()
            .engine(Arc::new(Reporting::holding(
                &[],
                Lifecycle::Exited,
                Health::None,
            )))
            .settings(settings)
            .build();
        let command = Command::Down {
            forms: vec!["search".to_owned()],
            wait: Waiting::ForTheDownloads,
        };
        let produced = report(dispatch(command, &ctx).await);

        assert_eq!(
            produced.map(|report| (report.action, report.status)),
            Some(("down".to_owned(), Some(0)))
        );
    }

    #[tokio::test]
    async fn an_engine_that_will_not_start_is_reported_to_the_operator() {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        let ctx = a_context()
            .runner(Arc::new(Scripted(Err(Failure::NotFound {
                program: "docker".to_owned(),
            }))))
            .engine(Arc::new(Reporting::default()))
            .settings(settings)
            .build();
        let command = Command::Pull {
            forms: vec!["library".to_owned()],
        };
        let refusal = dispatch(command, &ctx)
            .await
            .err()
            .map(|problem| problem.code);
        assert_eq!(refusal, Some(crate::ports::process::MISSING_PROGRAM));
    }

    #[tokio::test]
    async fn pull_progress_streams_composes_output_line_by_line_then_the_exit() {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        let ctx = a_context()
            .runner(Arc::new(Scripted(Ok(Output {
                status: Some(0),
                stdout: "library Pulling\nlibrary Pulled\n".to_owned(),
                stderr: String::new(),
            }))))
            .engine(Arc::new(Reporting::default()))
            .settings(settings)
            .build();

        let (closed, silent) = tokio::sync::mpsc::channel(1);
        drop(closed);
        let mut progress = pull_progress(&ctx, &["library".to_owned()])
            .await
            .unwrap_or(silent);
        let mut lines = Vec::new();
        let mut status = None;
        while let Some(event) = progress.recv().await {
            match event {
                Progress::Line(line) => lines.push(line),
                Progress::Ended(code) => status = code,
            }
        }
        // Each of Compose's per-image lines arrives on the stream, then the exit —
        // what a surface renders as it happens rather than after.
        assert_eq!(
            lines,
            vec!["library Pulling".to_owned(), "library Pulled".to_owned()]
        );
        assert_eq!(status, Some(0));
    }

    #[tokio::test]
    async fn a_pull_that_cannot_spawn_compose_is_a_problem_not_a_stream() {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        let ctx = a_context()
            .runner(Arc::new(Scripted(Err(Failure::NotFound {
                program: "docker".to_owned(),
            }))))
            .engine(Arc::new(Reporting::default()))
            .settings(settings)
            .build();
        let refusal = pull_progress(&ctx, &["library".to_owned()])
            .await
            .err()
            .map(|problem| problem.code);
        assert_eq!(refusal, Some(crate::ports::process::MISSING_PROGRAM));
    }

    #[tokio::test]
    async fn restarting_names_the_services_and_nothing_else() {
        let ctx = rehearsing(crate::config::Protocols::both());
        let command = Command::Restart {
            forms: vec!["library".to_owned()],
            services: vec!["jellyfin".to_owned()],
        };
        let produced = report(dispatch(command, &ctx).await);

        assert_eq!(
            produced.map(|report| (report.action, report.command.last().cloned())),
            Some(("restart".to_owned(), Some("jellyfin".to_owned())))
        );
    }

    #[tokio::test]
    async fn a_form_this_stack_does_not_have_never_reaches_the_engine() {
        let ctx = rehearsing(crate::config::Protocols::both());
        let command = Command::Up {
            forms: vec!["telly".to_owned()],
        };
        let outcome = dispatch(command, &ctx).await;
        assert_eq!(
            outcome.as_ref().err().map(|problem| problem.code),
            Some(crate::stack::closure::NO_SUCH_FORM)
        );
        assert_eq!(report(outcome), None, "nothing ran, so there is no report");
    }

    #[tokio::test]
    async fn an_unreadable_stack_is_reported_before_anything_is_started() {
        let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
        let ctx = a_context()
            .engine(Arc::new(Reporting::default()))
            .over(nowhere)
            .build();
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };
        assert_eq!(
            dispatch(command, &ctx).await.err().map(|p| p.code),
            Some(crate::stack::STACK_UNREADABLE)
        );
    }

    #[tokio::test]
    async fn an_embedded_stack_with_nowhere_to_go_stops_before_starting_anything() {
        static EMBEDDED: include_dir::Dir<'_> =
            include_dir::include_dir!("$CARGO_MANIFEST_DIR/../../assets/media-stack");

        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            stack_dir: None,
            ..Settings::default()
        };
        let ctx = a_context()
            .engine(Arc::new(Reporting::default()))
            .over(Source::Embedded(&EMBEDDED))
            .settings(settings)
            .build();
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };
        assert_eq!(
            dispatch(command, &ctx).await.err().map(|p| p.code),
            Some(crate::stack::STACK_NOT_SET_UP),
            "an operator who has not run setup is told to, not shown a path error"
        );
    }

    #[tokio::test]
    async fn a_stack_that_contradicts_itself_is_refused_with_every_fault_at_once() {
        let invalid = Source::External(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/invalid"
        )));
        let ctx = a_context()
            .engine(Arc::new(Reporting::default()))
            .over(invalid)
            .build();
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };

        let problem = dispatch(command, &ctx).await.err();
        assert_eq!(
            problem.as_ref().map(|problem| problem.code),
            Some(crate::stack::STACK_INVALID)
        );

        let detail = problem
            .and_then(|problem| problem.detail)
            .unwrap_or_default();
        for expected in [
            "names profile telly, which is not declared",
            "that is not a pin",
            "not a recognised OSI identifier",
        ] {
            assert!(
                detail.contains(expected),
                "missing {expected:?} in: {detail}"
            );
        }
    }

    /// A context whose settings live in a scratch file.
    fn with_config(path: &std::path::Path) -> Ctx {
        let settings = Settings {
            env_file: Some(path.to_path_buf()),
            ..Settings::default()
        };
        a_context()
            .engine(Arc::new(Reporting::default()))
            .settings(settings)
            .build()
    }

    fn config_scratch(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-app-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join(".env")
    }

    fn settings_of(outcome: Result<Outcome, Box<super::Problem>>) -> Option<Vec<(String, String)>> {
        match outcome {
            Ok(Outcome::Config(report)) => Some(
                report
                    .settings
                    .into_iter()
                    .map(|setting| (setting.key, setting.value))
                    .collect(),
            ),
            Ok(
                Outcome::Version(_)
                | Outcome::Forms(_)
                | Outcome::Preview(_)
                | Outcome::Lifecycle(_)
                | Outcome::Quality(_)
                | Outcome::Upgrade(_)
                | Outcome::Music(_)
                | Outcome::Trace(_)
                | Outcome::Household(_)
                | Outcome::FrontDoor(_)
                | Outcome::Stuck(_)
                | Outcome::Word(_)
                | Outcome::Glossary(_)
                | Outcome::Clients(_)
                | Outcome::Invited(_)
                | Outcome::Removed(_)
                | Outcome::Outbound(_)
                | Outcome::Stored(_)
                | Outcome::Space(_)
                | Outcome::Status(_)
                | Outcome::Doctor(_)
                | Outcome::Repair(_)
                | Outcome::Undo(_)
                | Outcome::Seed(_)
                | Outcome::Reset(_)
                | Outcome::Wizard(_)
                | Outcome::Backup(_)
                | Outcome::Support(_)
                | Outcome::Archives(_)
                | Outcome::Restore(_)
                | Outcome::Watch(_)
                | Outcome::Walkthrough(_),
            )
            | Err(_) => None,
        }
    }

    #[tokio::test]
    async fn a_setting_can_be_written_and_read_back() {
        let path = config_scratch("round-trip");
        let ctx = with_config(&path);

        let written = dispatch(
            Command::ConfigSet {
                key: "LEMONFIBER_USENET".to_owned(),
                value: "on".to_owned(),
            },
            &ctx,
        )
        .await;
        assert_eq!(
            settings_of(written),
            Some(vec![("LEMONFIBER_USENET".to_owned(), "on".to_owned())])
        );

        let read = dispatch(
            Command::ConfigGet {
                key: "LEMONFIBER_USENET".to_owned(),
            },
            &ctx,
        )
        .await;
        assert_eq!(
            settings_of(read),
            Some(vec![("LEMONFIBER_USENET".to_owned(), "on".to_owned())])
        );

        let _ = std::fs::remove_dir_all(path.parent().unwrap_or(std::path::Path::new("/")));
    }

    #[tokio::test]
    async fn a_rehearsed_change_reports_itself_and_writes_nothing() {
        let path = config_scratch("rehearsed");
        let ctx = with_config(&path).rehearsing();

        let outcome = dispatch(
            Command::ConfigSet {
                key: "LEMONFIBER_TORRENT".to_owned(),
                value: "on".to_owned(),
            },
            &ctx,
        )
        .await;
        assert!(
            matches!(&outcome, Ok(Outcome::Config(report)) if report.changed && report.rehearsed),
            "a rehearsal reports the change it would make, and that it was a rehearsal"
        );
        assert!(!path.exists(), "a rehearsal writes nothing");
    }

    #[tokio::test]
    async fn a_lifecycle_command_with_a_config_file_reports_no_edits_for_an_external_stack() {
        // With an environment file in hand, a lifecycle command derives where it would
        // keep the materialised-stack record beside it. The stack here is external —
        // the operator's own on disk — so nothing is written and no edit is reported.
        let path = config_scratch("lifecycle-config");
        let ctx = with_config(&path).rehearsing();
        let outcome = dispatch(
            Command::Up {
                forms: vec!["tv".to_owned()],
            },
            &ctx,
        )
        .await;
        let edits = report(outcome)
            .map(|report| report.stack_edits)
            .unwrap_or_default();
        assert!(
            edits.is_empty(),
            "an external stack is left as it is, so nothing is reported"
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap_or(std::path::Path::new("/")));
    }

    /// A rehearsing context whose engine can be reached and reports the named services
    /// up. Reachable matters here in a way it does not for the other lifecycle
    /// commands: a switch decides what to move by asking what is running, so an engine
    /// that refuses the question stops it before it has anything to say.
    fn switching(up: &[&str]) -> Ctx {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        a_context()
            .engine(Arc::new(Reporting::holding(
                up,
                Lifecycle::Running,
                Health::Healthy,
            )))
            .settings(settings)
            .build()
            .rehearsing()
    }

    #[tokio::test]
    async fn a_switch_onto_a_cold_stack_starts_the_closure_and_stops_nothing() {
        let ctx = switching(&[]);
        let switched = report(
            dispatch(
                Command::Switch {
                    forms: vec!["tv".to_owned()],
                },
                &ctx,
            )
            .await,
        )
        .and_then(|report| report.switched);
        assert!(
            switched
                .as_ref()
                .is_some_and(|moved| moved.stopped.is_empty()
                    && moved.kept.is_empty()
                    && !moved.started.is_empty()
                    && moved.stop_command.is_none()),
            "nothing is up, so there is nothing to stop and no command to stop it with: \
             {switched:?}"
        );
    }

    #[tokio::test]
    async fn a_switch_stops_what_left_the_closure_and_keeps_what_stayed() {
        // Both are up; `library` holds the one and not the other. That difference is
        // the whole of narrowing, so it is asserted through dispatch rather than only
        // against the function that decides it.
        let ctx = switching(&["jellyfin", "qbittorrent"]);

        let switched = report(
            dispatch(
                Command::Switch {
                    forms: vec!["library".to_owned()],
                },
                &ctx,
            )
            .await,
        )
        .and_then(|report| report.switched);

        assert_eq!(
            switched.as_ref().map(|moved| moved.stopped.clone()),
            Some(vec!["qbittorrent".to_owned()]),
            "only what fell outside the new closure is stopped: {switched:?}"
        );
        assert!(
            switched
                .as_ref()
                .is_some_and(|moved| moved.kept.contains(&"jellyfin".to_owned())),
            "and what both shapes hold keeps running: {switched:?}"
        );
        assert!(
            switched
                .as_ref()
                .is_some_and(|moved| moved
                    .stop_command
                    .as_ref()
                    .is_some_and(|command| command.contains(&"--profile".to_owned())
                        && command.contains(&"torrent".to_owned())
                        && command.contains(&"stop".to_owned()))),
            "the stop names the profile it is leaving, or Compose will not accept the \
             service: {switched:?}"
        );
    }

    /// The whole of it, for real rather than rehearsed: the stop runs, the start runs
    /// after it, and the switch waits for what it started the way starting a form
    /// does. Everything `library` holds is reported up, so the wait has nothing to
    /// wait for and the run reaches its own end rather than the patience deadline.
    #[tokio::test]
    async fn a_switch_that_succeeds_stops_starts_and_then_waits_for_health() {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        let ctx = a_context()
            .runner(Arc::new(Scripted(Ok(spoke("")))))
            .engine(Arc::new(Reporting::holding(
                &[
                    "jellyfin",
                    "seerr",
                    "calibre-web-automated",
                    "audiobookshelf",
                    "qbittorrent",
                ],
                Lifecycle::Running,
                Health::Healthy,
            )))
            .settings(settings)
            .build();

        let report = report(
            dispatch(
                Command::Switch {
                    forms: vec!["library".to_owned()],
                },
                &ctx,
            )
            .await,
        );

        assert_eq!(
            report.as_ref().and_then(|report| report.status),
            Some(0),
            "the status reported is the start's, not the stop's: {report:?}"
        );
        assert!(
            report
                .as_ref()
                .is_some_and(|report| report.condition.is_some() && !report.services.is_empty()),
            "and it waited, so it can say what the form came to: {report:?}"
        );
        assert!(
            report.as_ref().is_some_and(|report| report
                .switched
                .as_ref()
                .is_some_and(|moved| moved.stopped == vec!["qbittorrent".to_owned()])),
            "{report:?}"
        );
    }

    /// A context whose Compose cannot be run at all, holding the named services up.
    ///
    /// Distinct from a Compose that ran and refused: one is a stack that said no, the
    /// other is a machine with no Compose on it, and an operator can only act on the
    /// second by installing something.
    fn without_compose(up: &[&str]) -> Ctx {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        a_context()
            .runner(Arc::new(Scripted(Err(Failure::NotFound {
                program: "docker".to_owned(),
            }))))
            .engine(Arc::new(Reporting::holding(
                up,
                Lifecycle::Running,
                Health::Healthy,
            )))
            .settings(settings)
            .build()
    }

    /// Both invocations a switch makes can fail to run rather than fail to work, and
    /// the two arrive by different paths — the first while stopping what fell outside,
    /// the second while starting what the new shape holds.
    #[tokio::test]
    async fn a_switch_that_cannot_run_the_stop_says_so() {
        let refusal = dispatch(
            Command::Switch {
                forms: vec!["library".to_owned()],
            },
            // Something is up and outside `library`, so the stop is the first thing run.
            &without_compose(&["qbittorrent"]),
        )
        .await
        .err()
        .map(|problem| problem.code);

        assert_eq!(refusal, Some(crate::ports::process::MISSING_PROGRAM));
    }

    #[tokio::test]
    async fn a_switch_that_cannot_run_the_start_says_so() {
        let refusal = dispatch(
            Command::Switch {
                forms: vec!["library".to_owned()],
            },
            // Nothing is up, so there is nothing to stop and the start is run first.
            &without_compose(&[]),
        )
        .await
        .err()
        .map(|problem| problem.code);

        assert_eq!(refusal, Some(crate::ports::process::MISSING_PROGRAM));
    }

    /// A start that Compose refuses is reported as it stands rather than waited on:
    /// there is nothing to wait for, and a run that said what the form came to would
    /// be describing a form that never came to anything.
    #[tokio::test]
    async fn a_switch_whose_start_fails_reports_it_without_waiting() {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        let ctx = a_context()
            .runner(Arc::new(Scripted(Ok(refused("no such image")))))
            // Nothing is up, so nothing is stopped and the start is the only thing run.
            .engine(Arc::new(Reporting::holding(
                &[],
                Lifecycle::Running,
                Health::Healthy,
            )))
            .settings(settings)
            .build();

        let report = report(
            dispatch(
                Command::Switch {
                    forms: vec!["library".to_owned()],
                },
                &ctx,
            )
            .await,
        );

        assert_eq!(report.as_ref().and_then(|report| report.status), Some(1));
        assert!(
            report
                .as_ref()
                .is_some_and(|report| report.condition.is_none() && report.services.is_empty()),
            "nothing started, so there is nothing it could have waited for: {report:?}"
        );
    }

    /// Starting the new set over one that would not go down is how two shapes of the
    /// stack come to be running at once, so a stop that fails ends the switch there.
    #[tokio::test]
    async fn a_switch_whose_stop_fails_does_not_go_on_to_start_anything() {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        let ctx = a_context()
            .runner(Arc::new(Scripted(Ok(refused("that one is busy")))))
            .engine(Arc::new(Reporting::holding(
                &["qbittorrent"],
                Lifecycle::Running,
                Health::Healthy,
            )))
            .settings(settings)
            .build();

        let report = report(
            dispatch(
                Command::Switch {
                    forms: vec!["library".to_owned()],
                },
                &ctx,
            )
            .await,
        );

        assert_eq!(
            report.as_ref().and_then(|report| report.status),
            Some(1),
            "the failure the stop reported is what the switch reports"
        );
        assert!(
            report
                .as_ref()
                .is_some_and(|report| report.services.is_empty() && report.condition.is_none()),
            "and nothing was started, so there is nothing to have waited on: {report:?}"
        );
    }

    /// A switch works out what to move by asking the engine what is running, so one it
    /// cannot reach leaves it with nothing to say — even rehearsing. Refusing is the
    /// honest answer: reporting "would stop: nothing" would be a claim about a stack it
    /// never managed to look at. This is the one lifecycle command a rehearsal cannot
    /// answer without a daemon, and it is worth the difference.
    #[tokio::test]
    async fn a_switch_that_cannot_reach_the_engine_refuses_rather_than_guessing() {
        let refusal = dispatch(
            Command::Switch {
                forms: vec!["library".to_owned()],
            },
            &rehearsing(crate::config::Protocols::both()),
        )
        .await
        .err()
        .map(|problem| problem.code);

        assert_eq!(
            refusal,
            Some(crate::ports::docker::ENGINE_UNREACHABLE),
            "an engine that will not answer stops the switch rather than shrinking it"
        );
    }

    /// The published shape, pinned so a rename here breaks a test rather than a script.
    #[tokio::test]
    async fn a_switch_publishes_what_it_moved_under_the_names_a_script_reads() {
        let ctx = switching(&["jellyfin"]);
        let json = report(
            dispatch(
                Command::Switch {
                    forms: vec!["library".to_owned()],
                },
                &ctx,
            )
            .await,
        )
        .and_then(|report| serde_json::to_string(&report).ok());

        assert!(
            json.as_ref()
                .is_some_and(|json| json.contains("\"switched\"")
                    && json.contains("\"stopped\"")
                    && json.contains("\"started\"")
                    && json.contains("\"kept\"")),
            "{json:?}"
        );
    }

    #[tokio::test]
    async fn showing_settings_withholds_credentials() {
        let path = config_scratch("secrets");
        let ctx = with_config(&path);
        for (key, value) in [("DATA_ROOT", "/media"), ("WIREGUARD_PRIVATE_KEY", "abc123")] {
            let _ = dispatch(
                Command::ConfigSet {
                    key: key.to_owned(),
                    value: value.to_owned(),
                },
                &ctx,
            )
            .await;
        }

        let shown = settings_of(dispatch(Command::ConfigShow, &ctx).await).unwrap_or_default();
        assert_eq!(
            shown,
            vec![
                ("DATA_ROOT".to_owned(), "/media".to_owned()),
                (
                    "WIREGUARD_PRIVATE_KEY".to_owned(),
                    crate::config::store::REDACTED.to_owned()
                ),
            ]
        );

        let _ = std::fs::remove_dir_all(path.parent().unwrap_or(std::path::Path::new("/")));
    }

    #[tokio::test]
    async fn a_configuration_answer_serialises_under_its_own_kind() {
        let path = config_scratch("envelope");
        let ctx = with_config(&path);
        let _ = dispatch(
            Command::ConfigSet {
                key: "DATA_ROOT".to_owned(),
                value: "/media".to_owned(),
            },
            &ctx,
        )
        .await;

        let rendered = dispatch(Command::ConfigShow, &ctx)
            .await
            .ok()
            .map(Outcome::envelope)
            .and_then(|envelope| envelope.to_json().map(|json| (envelope.kind, json)));

        assert_eq!(
            rendered.map(|(kind, json)| (
                kind,
                json.starts_with(r#"{"api_version":1,"kind":"config","data":{"settings":["#),
                json.contains(r#""changed":false"#)
            )),
            Some((crate::model::kind::CONFIG, true, true))
        );

        let _ = std::fs::remove_dir_all(path.parent().unwrap_or(std::path::Path::new("/")));
    }

    #[tokio::test]
    async fn asking_a_non_configuration_outcome_for_settings_gets_none() {
        let ctx = ctx(Ok(spoke("v2.32.1")));
        assert_eq!(settings_of(dispatch(Command::Version, &ctx).await), None);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn settings_that_cannot_be_saved_reach_the_operator() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = std::env::temp_dir().join(format!("lemonfiber-app-{}-ro", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500));

        let ctx = with_config(&dir.join(".env"));
        let refusal = dispatch(
            Command::ConfigSet {
                key: "A".to_owned(),
                value: "1".to_owned(),
            },
            &ctx,
        )
        .await
        .err()
        .map(|problem| problem.code);
        assert_eq!(refusal, Some(crate::config::store::CONFIG_NOT_WRITTEN));

        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn settings_that_cannot_be_read_reach_the_operator() {
        // A file where the directory holding settings should be.
        let blocker =
            std::env::temp_dir().join(format!("lemonfiber-app-{}-blocked", std::process::id()));
        let _ = std::fs::remove_dir_all(&blocker);
        let _ = std::fs::write(&blocker, "in the way");

        let ctx = with_config(&blocker.join(".env"));
        let refusal = dispatch(Command::ConfigShow, &ctx)
            .await
            .err()
            .map(|problem| problem.code);
        assert_eq!(refusal, Some(crate::config::store::CONFIG_UNREADABLE));

        let _ = std::fs::remove_file(&blocker);
    }

    #[tokio::test]
    async fn settings_with_nowhere_to_live_say_setup_has_not_run() {
        let ctx = a_context().engine(Arc::new(Reporting::default())).build();
        assert_eq!(
            dispatch(Command::ConfigShow, &ctx)
                .await
                .err()
                .map(|p| p.code),
            Some(crate::config::store::CONFIG_NOWHERE)
        );
    }

    #[tokio::test]
    async fn a_lifecycle_outcome_serialises_under_its_own_kind() {
        let ctx = rehearsing(crate::config::Protocols::both());
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };
        let rendered = dispatch(command, &ctx)
            .await
            .ok()
            .map(Outcome::envelope)
            .and_then(|envelope| envelope.to_json().map(|json| (envelope.kind, json)));

        assert_eq!(
            rendered.map(|(kind, json)| (
                kind,
                json.starts_with(r#"{"api_version":1,"kind":"lifecycle","data":{"action":"up""#),
                json.contains(r#""rehearsed":true"#)
            )),
            Some((crate::model::kind::LIFECYCLE, true, true))
        );
    }

    /// A real run against an engine reporting whatever the test put in it.
    fn watching(engine: Reporting) -> Ctx {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        a_context()
            .engine(Arc::new(engine))
            .settings(settings)
            .build()
            // An HTTP port that answers nothing, so a diagnostic check reaching one — the
            // guide-source probe, a credential — resolves to unreachable rather than the
            // real network. Keeps the doctor tests self-contained and offline.
            .with_http(Fake::scripted(Vec::new()))
    }

    /// Everything the `library` form declares.
    const LIBRARY: [&str; 4] = [
        "jellyfin",
        "seerr",
        "calibre-web-automated",
        "audiobookshelf",
    ];

    #[tokio::test]
    async fn starting_waits_until_the_services_are_usable() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
        let ctx = watching(engine);
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };

        let produced = report(dispatch(command, &ctx).await);
        assert_eq!(
            produced.map(|report| (
                report.condition,
                report.services.len(),
                report
                    .services
                    .iter()
                    .all(|service| service.state == ServiceState::Healthy)
            )),
            Some((Some(Condition::Active), LIBRARY.len(), true)),
            "started means every service answered, not that a process exists"
        );
    }

    #[tokio::test]
    async fn starting_keeps_asking_until_the_services_are_ready() {
        // Unsettled on the first two listings and healthy on the third, which
        // is what a stack that is genuinely starting looks like. A gate that
        // only ever read the engine once would pass this test by luck and fail
        // every real start.
        let engine =
            Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Starting).settling_after(2);
        let ctx = watching(engine);
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };

        let produced = report(dispatch(command, &ctx).await);
        assert_eq!(
            produced.map(|report| report.condition),
            Some(Some(Condition::Active)),
            "waiting is the point: the answer changed while it waited"
        );
    }

    /// The requirement stated directly, and the only way to state it: "nothing was
    /// torn down" is a claim about commands that were never issued, so it is asserted
    /// against everything the runner was handed rather than against what came back.
    #[tokio::test]
    async fn one_service_failing_to_start_never_takes_down_the_rest() {
        let runner = Arc::new(Recording::answering(Ok(spoke(""))));
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        let ctx = a_context()
            .runner(runner.clone())
            .engine(Arc::new(Reporting::holding(
                &LIBRARY,
                Lifecycle::Running,
                Health::Starting,
            )))
            .settings(settings)
            .build()
            .with_http(Fake::scripted(Vec::new()))
            .waiting(Duration::ZERO);

        let refused = dispatch(
            Command::Up {
                forms: vec!["library".to_owned()],
            },
            &ctx,
        )
        .await
        .err();

        assert_eq!(
            refused.as_ref().map(|problem| problem.code),
            Some(super::NEVER_SETTLED),
            "the start is reported as not having finished"
        );
        assert!(
            runner.ran("up"),
            "it did try to start the form, so the claim below is about a real run"
        );
        assert!(
            !runner.ran("down"),
            "and never tore it down again for the one service that would not settle"
        );
        assert!(!runner.ran("stop"), "nor stopped what had already started");
    }

    /// A report about a container is a report about the wrong thing. What the operator
    /// lost is what the stack says the service was there to do.
    #[tokio::test]
    async fn a_service_that_will_not_start_says_what_its_absence_costs() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Starting);
        let refused = dispatch(
            Command::Up {
                forms: vec!["library".to_owned()],
            },
            &watching(engine).waiting(Duration::ZERO),
        )
        .await
        .err();

        assert_eq!(
            refused.as_ref().map(|problem| problem
                .meaning
                .contains("Files on disk, no way to watch them")),
            Some(true),
            "the manifest's own words for what jellyfin is for: {refused:?}"
        );
        assert_eq!(
            refused
                .as_ref()
                .map(|problem| problem.meaning.contains("left alone")),
            Some(true),
            "and the operator is told the rest of the form was not taken down with it"
        );
    }

    #[tokio::test]
    async fn a_service_that_never_becomes_usable_stops_the_start_and_says_which() {
        // A container that is running but still inside its start period is
        // exactly the case a process check would have called success.
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Starting)
            .saying("jellyfin", "Cannot open database, disk is full");
        let ctx = watching(engine).waiting(Duration::ZERO);
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };

        let refused = dispatch(command, &ctx).await.err();
        assert_eq!(
            refused.as_ref().map(|problem| problem.code),
            Some(super::NEVER_SETTLED)
        );
        assert_eq!(
            refused
                .as_ref()
                .map(|problem| problem.summary.contains("jellyfin")),
            Some(true),
            "the operator is told which service, not that something went wrong"
        );
        assert_eq!(
            refused
                .and_then(|problem| problem.detail)
                .map(|detail| detail.contains("disk is full")),
            Some(true),
            "the explanation is already on screen rather than left to be found"
        );
    }

    #[tokio::test]
    async fn a_service_that_will_not_start_and_says_nothing_still_reports_which() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Starting);
        let ctx = watching(engine).waiting(Duration::ZERO);
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };

        let refused = dispatch(command, &ctx).await.err();
        assert_eq!(
            refused.map(|problem| (problem.code, problem.detail)),
            Some((super::NEVER_SETTLED, None)),
            "silence is reported as silence rather than as an empty quotation"
        );
    }

    #[tokio::test]
    async fn a_crash_loop_is_not_something_starting_waits_out() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Restarting, Health::None);
        let ctx = watching(engine).waiting(Duration::from_secs(3600));
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };

        // Patience of an hour, and this must still return at once: a loop has
        // settled, and waiting for it is waiting forever.
        let produced = report(dispatch(command, &ctx).await);
        assert_eq!(
            produced.map(|report| report.condition),
            Some(Some(Condition::Degraded))
        );
    }

    /// The requirement: an operator who started two overlapping forms and stops one
    /// of them is not asking for the other to lose its services. The refusal names
    /// the form, because one told only "cannot stop" cannot act on it.
    #[tokio::test]
    async fn stopping_a_form_another_running_one_needs_is_refused_by_name() {
        // Everything `tv` and `movies` both hold, plus the one service each has of its
        // own — so both are up in their own right and neither contains the other.
        let running = Reporting::holding(
            &[
                "flaresolverr",
                "nzbhydra2",
                "prowlarr",
                "sabnzbd",
                "gluetun",
                "qbittorrent",
                "bazarr",
                "sonarr",
                "radarr",
            ],
            Lifecycle::Running,
            Health::Healthy,
        );
        let refused = dispatch(
            Command::Down {
                forms: vec!["tv".to_owned()],
                wait: Waiting::Never,
            },
            &watching(running),
        )
        .await
        .err();

        assert_eq!(
            refused.as_ref().map(|problem| problem.code),
            Some(super::STILL_NEEDED)
        );
        assert!(
            refused
                .as_ref()
                .is_some_and(|problem| problem.summary.contains("movies")),
            "the operator is told which form still needs it: {refused:?}"
        );
    }

    /// Stopping asks what else is running before it acts, so an engine that will not
    /// answer stops the stop. It cannot be overruled into taking down something it was
    /// never able to see — and this is the one lifecycle command where that is a new
    /// requirement, which makes it worth saying plainly rather than discovering.
    #[tokio::test]
    async fn stopping_reports_an_engine_it_cannot_see() {
        let refusal = dispatch(
            Command::Down {
                forms: vec!["library".to_owned()],
                wait: Waiting::Never,
            },
            &rehearsing(crate::config::Protocols::both()),
        )
        .await
        .err()
        .map(|problem| problem.code);

        assert_eq!(refusal, Some(crate::ports::docker::ENGINE_UNREACHABLE));
    }

    /// The ordinary case, and the one that must not be made harder: one form up, that
    /// form stopped, nothing else running to be deprived of anything.
    #[tokio::test]
    async fn stopping_the_only_form_that_is_up_is_not_refused() {
        let running = Reporting::holding(
            &[
                "jellyfin",
                "seerr",
                "calibre-web-automated",
                "audiobookshelf",
            ],
            Lifecycle::Running,
            Health::Healthy,
        );
        let produced = report(
            dispatch(
                Command::Down {
                    forms: vec!["library".to_owned()],
                    wait: Waiting::Never,
                },
                &watching(running),
            )
            .await,
        );

        assert_eq!(
            produced.map(|report| report.action),
            Some("down".to_owned()),
            "nothing else holds what `library` holds, so it simply stops"
        );
    }

    #[tokio::test]
    async fn stopping_does_not_wait_for_anything() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Starting);
        let ctx = watching(engine).waiting(Duration::ZERO);
        let command = Command::Down {
            forms: vec!["library".to_owned()],
            wait: Waiting::Never,
        };

        let produced = report(dispatch(command, &ctx).await);
        assert_eq!(
            produced.map(|report| (report.condition, report.services.is_empty())),
            Some((None, true)),
            "stopping is finished when Compose says so"
        );
    }

    /// Stopping named services is a different request from tearing a form down, and
    /// it reaches Compose as a different word — `stop`, which leaves them where they
    /// are, rather than `down`, which removes what the form started.
    #[tokio::test]
    async fn stopping_named_services_stops_only_those() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
        let ctx = watching(engine).waiting(Duration::ZERO);
        let command = Command::Halt {
            forms: vec!["library".to_owned()],
            services: vec!["sonarr".to_owned()],
        };

        let produced = report(dispatch(command, &ctx).await);
        assert_eq!(
            produced
                .as_ref()
                .map(|report| report.action.clone())
                .as_deref(),
            Some("stop")
        );
        assert!(
            produced.is_some_and(|report| report.command.ends_with(&[
                "stop".to_owned(),
                "--".to_owned(),
                "sonarr".to_owned()
            ])),
            "the named service is fenced off from option parsing"
        );
    }

    #[tokio::test]
    async fn a_compose_invocation_that_failed_is_not_then_waited_on() {
        let settings = Settings {
            protocols: crate::config::Protocols::both(),
            ..Settings::default()
        };
        let ctx = a_context()
            .runner(Arc::new(Scripted(Ok(refused("no such image")))))
            .engine(Arc::new(Reporting::holding(
                &LIBRARY,
                Lifecycle::Running,
                Health::Starting,
            )))
            .settings(settings)
            .build()
            .waiting(Duration::ZERO);

        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };
        let produced = report(dispatch(command, &ctx).await);
        assert_eq!(
            produced.map(|report| (report.status, report.condition)),
            Some((Some(1), None)),
            "waiting for health after Compose refused would report the wrong fault"
        );
    }

    #[tokio::test]
    async fn starting_reports_an_engine_it_cannot_see() {
        let ctx = watching(Reporting::absent());
        let command = Command::Up {
            forms: vec!["library".to_owned()],
        };
        assert_eq!(
            dispatch(command, &ctx).await.err().map(|p| p.code),
            Some(crate::ports::docker::ENGINE_UNREACHABLE)
        );
    }

    /// The status a survey produced, as pairs of service and state.
    fn stated(
        outcome: Result<Outcome, Box<super::Problem>>,
    ) -> Option<Vec<(String, ServiceState)>> {
        match outcome {
            Ok(Outcome::Status(report)) => Some(
                report
                    .services
                    .into_iter()
                    .map(|service| (service.id, service.state))
                    .collect(),
            ),
            Ok(
                Outcome::Version(_)
                | Outcome::Forms(_)
                | Outcome::Preview(_)
                | Outcome::Lifecycle(_)
                | Outcome::Config(_)
                | Outcome::Quality(_)
                | Outcome::Upgrade(_)
                | Outcome::Music(_)
                | Outcome::Trace(_)
                | Outcome::Household(_)
                | Outcome::FrontDoor(_)
                | Outcome::Stuck(_)
                | Outcome::Word(_)
                | Outcome::Glossary(_)
                | Outcome::Clients(_)
                | Outcome::Invited(_)
                | Outcome::Removed(_)
                | Outcome::Outbound(_)
                | Outcome::Stored(_)
                | Outcome::Space(_)
                | Outcome::Doctor(_)
                | Outcome::Repair(_)
                | Outcome::Undo(_)
                | Outcome::Seed(_)
                | Outcome::Reset(_)
                | Outcome::Wizard(_)
                | Outcome::Backup(_)
                | Outcome::Support(_)
                | Outcome::Archives(_)
                | Outcome::Restore(_)
                | Outcome::Watch(_)
                | Outcome::Walkthrough(_),
            )
            | Err(_) => None,
        }
    }

    #[tokio::test]
    async fn a_non_status_outcome_has_no_services_to_report() {
        let ctx = ctx(Ok(spoke("v2.32.1")));
        assert_eq!(stated(dispatch(Command::Version, &ctx).await), None);
    }

    #[tokio::test]
    async fn asking_what_is_running_names_every_service_a_form_declares() {
        let engine = Reporting::holding(&["jellyfin"], Lifecycle::Running, Health::Healthy);
        let ctx = watching(engine);
        let command = Command::Ps {
            forms: vec!["library".to_owned()],
        };

        let seen = stated(dispatch(command, &ctx).await).unwrap_or_default();
        assert_eq!(seen.len(), LIBRARY.len());
        assert!(
            seen.iter()
                .any(|(id, state)| id == "jellyfin" && *state == ServiceState::Healthy),
            "{seen:?}"
        );
        assert!(
            seen.iter()
                .any(|(id, state)| id == "seerr" && *state == ServiceState::Absent),
            "a service that was never started is absent, not missing: {seen:?}"
        );
    }

    #[tokio::test]
    async fn asking_what_is_running_without_naming_a_form_covers_the_whole_stack() {
        let ctx = watching(Reporting::holding(&[], Lifecycle::Running, Health::None));
        let seen = stated(dispatch(Command::Ps { forms: Vec::new() }, &ctx).await);

        assert_eq!(
            seen.map(|services| services.len() > LIBRARY.len()),
            Some(true),
            "what is running is a question about the machine, not about a form"
        );
    }

    #[tokio::test]
    async fn asking_what_is_running_reports_an_engine_it_cannot_see() {
        let ctx = watching(Reporting::absent());
        let refusal = dispatch(Command::Ps { forms: Vec::new() }, &ctx)
            .await
            .err()
            .map(|problem| problem.code);
        assert_eq!(
            refusal,
            Some(crate::ports::docker::ENGINE_UNREACHABLE),
            "an unreachable engine is not a stack with nothing in it"
        );
    }

    #[tokio::test]
    async fn asking_about_a_form_this_stack_does_not_have_is_refused() {
        let ctx = watching(Reporting::default());
        let command = Command::Ps {
            forms: vec!["telly".to_owned()],
        };
        assert_eq!(
            dispatch(command, &ctx).await.err().map(|p| p.code),
            Some(crate::stack::closure::NO_SUCH_FORM)
        );
    }

    #[tokio::test]
    async fn asking_what_is_running_from_a_stack_that_cannot_be_read_is_refused() {
        let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
        let ctx = a_context()
            .engine(Arc::new(Reporting::default()))
            .over(nowhere)
            .build();
        assert_eq!(
            dispatch(Command::Ps { forms: Vec::new() }, &ctx)
                .await
                .err()
                .map(|problem| problem.code),
            Some(crate::stack::STACK_UNREADABLE),
            "an operator's own --stack-dir mistake reaches them here too"
        );
    }

    #[tokio::test]
    async fn a_status_serialises_under_its_own_kind() {
        let engine = Reporting::holding(&["jellyfin"], Lifecycle::Running, Health::Healthy);
        let ctx = watching(engine);
        let command = Command::Ps {
            forms: vec!["library".to_owned()],
        };

        let rendered = dispatch(command, &ctx)
            .await
            .ok()
            .map(Outcome::envelope)
            .and_then(|envelope| envelope.to_json().map(|json| (envelope.kind, json)));

        assert_eq!(
            rendered.map(|(kind, json)| (
                kind,
                json.starts_with(r#"{"api_version":1,"kind":"status","data":{"forms":["library"]"#),
                json.contains(r#""state":"healthy""#)
            )),
            Some((crate::model::kind::STATUS, true, true))
        );
    }

    /// The lines a log stream carried, in the order it handed them over.
    ///
    /// Shaped like [`heard`] beside it: a closed channel stands in for a stream that
    /// could not be opened, so there is no branch here for a test to leave unrun.
    async fn spoken(ctx: &Ctx, forms: &[String], query: LogQuery) -> Vec<String> {
        let (closed, silent) = tokio::sync::mpsc::channel(1);
        drop(closed);

        let mut lines = super::logs(ctx, forms, &[], query).await.unwrap_or(silent);

        let mut seen = Vec::new();
        while let Some(line) = lines.recv().await {
            seen.push(line.line);
        }
        seen
    }

    /// The services a log stream actually carried lines for.
    async fn heard(ctx: &Ctx, forms: &[String], services: &[String]) -> Vec<String> {
        let (closed, silent) = tokio::sync::mpsc::channel(1);
        drop(closed);

        let query = LogQuery::recent(10);
        let mut lines = super::logs(ctx, forms, services, query)
            .await
            .unwrap_or(silent);

        let mut seen = Vec::new();
        while let Some(line) = lines.recv().await {
            seen.push(line.service);
        }
        seen.sort();
        seen.dedup();
        seen
    }

    /// One reader per container means a scrollback arrives in bursts. Read back, it
    /// should be one account of what happened rather than three.
    #[tokio::test]
    async fn a_scrollback_reads_as_one_timeline_rather_than_one_burst_per_service() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
            .saying_at("jellyfin", "2026-08-21T19:00:03.000000000Z", "third")
            .saying_at("jellyfin", "2026-08-21T19:00:04.000000000Z", "fourth")
            .saying_at("seerr", "2026-08-21T19:00:01.000000000Z", "first")
            .saying_at("seerr", "2026-08-21T19:00:02.000000000Z", "second");

        let said = spoken(
            &watching(engine),
            &["library".to_owned()],
            LogQuery::recent(20),
        )
        .await;

        assert_eq!(
            said,
            ["first", "second", "third", "fourth"],
            "the containers' own stamps decide, not which reader finished first"
        );
    }

    /// A live stream has nothing to sort against, so it is handed straight back.
    #[tokio::test]
    async fn following_hands_the_stream_back_as_it_arrives() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
            .saying_at("jellyfin", "2026-08-21T19:00:09.000000000Z", "later")
            .saying_at("seerr", "2026-08-21T19:00:01.000000000Z", "earlier");

        let said = spoken(
            &watching(engine),
            &["library".to_owned()],
            LogQuery {
                tail: 20,
                follow: true,
            },
        )
        .await;

        assert_eq!(
            said,
            ["later", "earlier"],
            "arrival order is the only order a live stream has"
        );
    }

    #[tokio::test]
    async fn reading_logs_for_a_form_narrows_to_what_that_form_declares() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
            .saying("jellyfin", "started")
            .saying("sonarr", "also started");
        let ctx = watching(engine);

        assert_eq!(
            heard(&ctx, &["library".to_owned()], &[]).await,
            vec!["jellyfin".to_owned()],
            "a form's log view must not carry another form's output"
        );
    }

    #[tokio::test]
    async fn naming_a_service_narrows_further_still() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
            .saying("jellyfin", "started")
            .saying("seerr", "also started");
        let ctx = watching(engine);

        assert_eq!(
            heard(&ctx, &["library".to_owned()], &["seerr".to_owned()]).await,
            vec!["seerr".to_owned()]
        );
    }

    #[tokio::test]
    async fn naming_no_form_reads_everything_that_is_saying_anything() {
        let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
            .saying("jellyfin", "started")
            .saying("sonarr", "also started");
        let ctx = watching(engine);

        assert_eq!(
            heard(&ctx, &[], &[]).await,
            vec!["jellyfin".to_owned(), "sonarr".to_owned()]
        );
    }

    #[tokio::test]
    async fn reading_logs_reports_an_engine_it_cannot_see() {
        let ctx = watching(Reporting::absent());
        let query = LogQuery::recent(10);
        let refusal = super::logs(&ctx, &[], &[], query)
            .await
            .err()
            .map(|problem| problem.code);
        assert_eq!(refusal, Some(crate::ports::docker::ENGINE_UNREACHABLE));
    }

    #[tokio::test]
    async fn reading_logs_for_a_form_this_stack_does_not_have_is_refused() {
        let ctx = watching(Reporting::default());
        let query = LogQuery::recent(10);
        let refusal = super::logs(&ctx, &["telly".to_owned()], &[], query)
            .await
            .err()
            .map(|problem| problem.code);
        assert_eq!(refusal, Some(crate::stack::closure::NO_SUCH_FORM));
    }

    #[tokio::test]
    async fn reading_logs_from_a_stack_that_cannot_be_read_is_refused() {
        let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
        let ctx = a_context()
            .engine(Arc::new(Reporting::default()))
            .over(nowhere)
            .build();
        let query = LogQuery::recent(10);
        assert_eq!(
            super::logs(&ctx, &[], &[], query)
                .await
                .err()
                .map(|problem| problem.code),
            Some(crate::stack::STACK_UNREADABLE)
        );
    }

    #[tokio::test]
    async fn a_context_can_be_told_how_long_to_wait() {
        let ctx = watching(Reporting::default()).waiting(Duration::from_secs(7));
        assert_eq!(ctx.patience, Duration::from_secs(7));
    }

    #[tokio::test]
    async fn the_engine_these_tests_use_answers_the_whole_port() {
        // Worth asserting rather than assuming. A fake that answers a method
        // more agreeably than a real engine would makes the path it shortcuts
        // untestable, which is how the log stream's own failure case went
        // missing until it was written down here.
        let engine = Reporting::absent();

        let ran = engine.exec("gluetun", &["true".to_owned()]).await;
        assert!(
            matches!(&ran, Err(EngineFailure::NoSuchContainer { name }) if name == "gluetun"),
            "{ran:?}"
        );

        let sampled = engine.stats("lemonfiber").await;
        assert_eq!(
            sampled.ok().map(|mut samples| samples.try_recv().is_err()),
            Some(true),
            "nothing is sampled, and the stream says so by ending"
        );
    }

    #[test]
    fn a_rehearsing_context_changes_nothing_else() {
        let rehearsal = ctx(Ok(spoke(""))).rehearsing();
        assert!(rehearsal.dry_run);
    }
}
