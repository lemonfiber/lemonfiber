//! Where this copy of lemonfiber stands, through the dispatcher.
//!
//! From here rather than from a `#[cfg(test)]` module for the reason the disclosure
//! beside it is: the app layer is compiled twice, and a path exercised only in-crate
//! has its coverage counted from the copy that never ran.
//!
//! Nothing reaches a real filesystem and nothing reaches the network. The two seams
//! that matter here are exactly those — where the running binary is and what put it
//! there, and what the release list said — so both are scripted and the assertions
//! are on what the run *asked for* as much as on what it answered with. A check that
//! reported the right standing while quietly asking somewhere nobody was told about
//! would be the failure this whole family exists to prevent.

use std::path::PathBuf;
use std::sync::Arc;

use lemonfiber_adapters::{Daemon, Local};
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::config::{Reaching, Settings, REACH_UPDATES_KEY};
use lemonfiber_core::model::UpdateReport;
use lemonfiber_core::outbound::RELEASE_LIST;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::http::Http;
use lemonfiber_core::ports::FileSystem;
use lemonfiber_core::self_update::{receipt_under, Installed, Standing};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::ports::Stopped;
use lemonfiber_fixtures::program::Program;

/// A moment far enough from the epoch that a day can be subtracted from it.
const NOW: u64 = 1_700_000_000;

/// A day, which is how long the check leaves between attempts.
const A_DAY: u64 = 60 * 60 * 24;

/// This operator's home directory, where the installer leaves its receipt.
const HOME: &str = "/home/sam";

/// Where a copy the shell installer wrote sits by default, which is also cargo's.
const IN_CARGOS_BIN: &str = "/home/sam/.cargo/bin/lemonfiber";

/// A release list holding one published release at this version.
fn released(tag: &str) -> String {
    format!(r#"[{{"tag_name":"{tag}","draft":false}}]"#)
}

/// The settings a run reads: where the binary is, where home is, and what may be
/// reached. The record of past checks sits beside the settings file, so a run given
/// one has somewhere to keep what it read.
fn settings(program: Option<&str>) -> Settings {
    Settings {
        program: program.map(PathBuf::from),
        home: Some(PathBuf::from(HOME)),
        env_file: Some(PathBuf::from("/home/sam/.config/lemonfiber/.env")),
        ..Settings::default()
    }
}

/// Where a run keeps what the last few checks came to.
fn record() -> PathBuf {
    PathBuf::from("/home/sam/.config/lemonfiber/updates.json")
}

/// A record of checks as it is written down between runs.
fn remembered(asked: Option<u64>, offered: Option<&str>, quiet: u32) -> String {
    let asked = asked.map_or_else(|| "null".to_owned(), |at| at.to_string());
    let offered = offered.map_or_else(|| "null".to_owned(), |read| format!("\"{read}\""));
    format!(r#"{{"asked":{asked},"offered":{offered},"quiet":{quiet}}}"#)
}

/// The context a check runs against.
fn ctx(files: &Arc<Program>, http: &Arc<Fake>, settings: Settings) -> Ctx {
    Ctx::new(
        Arc::new(Local),
        Arc::new(Daemon::local()),
        Stopped::at(NOW),
        Arc::clone(files) as Arc<dyn FileSystem>,
        Source::External(std::path::Path::new("/lemonfiber/no/such/stack")),
        settings,
        Environment::MacOs,
    )
    .with_http(Arc::clone(http) as Arc<dyn Http>)
}

/// What the check answered, through the dispatcher rather than by calling it.
async fn asked(command: Command, ctx: &Ctx) -> UpdateReport {
    match dispatch(command, ctx).await {
        Ok(Outcome::SelfUpdate(report)) => report,
        other => unreachable!("the check answers with itself: {other:?}"),
    }
}

/// The ordinary run: a machine, a release list that answers, and nothing named.
async fn standing(files: &Arc<Program>, http: &Arc<Fake>, program: Option<&str>) -> UpdateReport {
    let ctx = ctx(files, http, settings(program));
    asked(Command::SelfUpdate { to: None }, &ctx).await
}

/// A copy under a package manager's own tree is that manager's to move, and the
/// operator is handed its command rather than left holding a fact.
#[tokio::test]
async fn a_copy_a_package_manager_owns_is_deferred_to_it_by_name() {
    let files = Program::ordinary().shared();
    let http = Fake::always(Answer::reply(200, released("v0.99.0")));

    let report = standing(
        &files,
        &http,
        Some("/opt/homebrew/Cellar/lemonfiber/0.13.0/bin/lemonfiber"),
    )
    .await;

    assert_eq!(report.standing, Standing::ManagedExternally);
    assert_eq!(report.installed, Installed::Homebrew);
    assert_eq!(report.owner.as_deref(), Some("Homebrew"));
    assert_eq!(report.command.as_deref(), Some("brew upgrade lemonfiber"));
    assert_eq!(report.instead, None);
    assert_eq!(report.offered.as_deref(), Some("0.99.0"));
}

/// And nothing probes beside a file somebody else owns. Whether it could be written
/// is beside the point, and an answer nothing may act on is one not worth asking for.
#[tokio::test]
async fn a_copy_somebody_else_owns_is_never_probed_for_writability() {
    let files = Program::ordinary().shared();
    let http = Fake::always(Answer::reply(200, released("v0.99.0")));

    let report = standing(&files, &http, Some("/usr/bin/lemonfiber")).await;

    assert_eq!(report.installed, Installed::Distribution);
    assert_eq!(report.replaceable, None);
    assert!(files.removed().is_empty(), "{:?}", files.removed());
}

/// The one case a path cannot answer: the shell installer writes into cargo's own
/// `bin`, so only the receipt beside it says which of the two put this copy here.
#[tokio::test]
async fn the_receipt_the_installer_leaves_is_what_tells_it_from_a_cargo_install() {
    let http = Fake::always(Answer::reply(200, released("v0.99.0")));

    let installed = Program::ordinary()
        .holding(receipt_under(std::path::Path::new(HOME)), "{}")
        .shared();
    let report = standing(&installed, &http, Some(IN_CARGOS_BIN)).await;
    assert_eq!(report.installed, Installed::Installer);
    assert_eq!(report.standing, Standing::UpdateAvailable);
    assert_eq!(report.owner, None);
    let typed = report.command.unwrap_or_default();
    assert!(typed.contains("releases/download/v0.99.0/"), "{typed}");

    let built = Program::ordinary()
        .holding(
            "/home/sam/.cargo/.crates2.json",
            r#"{"lemonfiber 0.13.0":{}}"#,
        )
        .shared();
    let report = standing(
        &built,
        &Fake::always(Answer::reply(200, released("v0.99.0"))),
        Some(IN_CARGOS_BIN),
    )
    .await;
    assert_eq!(report.installed, Installed::Cargo);
    let typed = report.command.unwrap_or_default();
    assert!(typed.contains("--tag v0.99.0"), "{typed}");
}

/// A record naming some other program is cargo's record of something else, and says
/// nothing about this copy.
#[tokio::test]
async fn a_cargo_record_that_does_not_name_this_program_claims_nothing_about_it() {
    let files = Program::ordinary()
        .holding("/home/sam/.cargo/.crates2.json", r#"{"ripgrep 14.0.0":{}}"#)
        .shared();
    let http = Fake::always(Answer::reply(200, released("v0.99.0")));

    let report = standing(&files, &http, Some(IN_CARGOS_BIN)).await;

    assert_eq!(report.installed, Installed::Elsewhere);
}

/// Following the link is what makes a package manager tellable at all: the name on
/// the path is in a directory anybody may write to, and the file it points at is not.
#[tokio::test]
async fn a_link_is_followed_so_the_tool_that_owns_the_file_can_be_told() {
    let files = Program::ordinary()
        .linked(
            "/usr/local/bin/lemonfiber",
            "/opt/homebrew/Cellar/lemonfiber/0.13.0/bin/lemonfiber",
        )
        .shared();
    let http = Fake::always(Answer::reply(200, released("v0.99.0")));

    let report = standing(&files, &http, Some("/usr/local/bin/lemonfiber")).await;

    assert_eq!(report.installed, Installed::Homebrew);
    assert_eq!(
        report.at.as_deref(),
        Some("/opt/homebrew/Cellar/lemonfiber/0.13.0/bin/lemonfiber")
    );
}

/// A machine that will not resolve the path falls back to the one it was given,
/// which still answers the question an operator asks of it: which file ran.
#[tokio::test]
async fn a_machine_that_resolves_nothing_still_names_the_file_that_ran() {
    let files = Program::ordinary().unresolvable().shared();
    let http = Fake::always(Answer::reply(200, released("v0.99.0")));

    let report = standing(&files, &http, Some("/opt/lemonfiber/lemonfiber")).await;

    assert_eq!(report.at.as_deref(), Some("/opt/lemonfiber/lemonfiber"));
    assert_eq!(report.installed, Installed::Elsewhere);
}

/// A machine that will not say where the running binary is says that, rather than
/// being guessed at — and a guess here would tell somebody to run a package manager
/// that never touched this copy.
#[tokio::test]
async fn a_machine_that_will_not_say_where_the_binary_is_is_not_guessed_at() {
    let files = Program::ordinary().shared();
    let http = Fake::always(Answer::reply(200, released("v0.99.0")));

    let report = standing(&files, &http, None).await;

    assert_eq!(report.at, None);
    assert_eq!(report.installed, Installed::Untellable);
    assert_eq!(report.replaceable, None);
}

/// A directory that will not take a new file is reported with the path, and the
/// report offers no way to become somebody who could.
#[tokio::test]
async fn a_directory_that_will_not_take_a_file_is_reported_with_the_path() {
    let files = Program::ordinary().unwritable().shared();
    let http = Fake::always(Answer::reply(200, released("v0.99.0")));

    let report = standing(&files, &http, Some("/opt/lemonfiber/lemonfiber")).await;

    assert_eq!(report.replaceable, Some(false));
    assert_eq!(report.at.as_deref(), Some("/opt/lemonfiber/lemonfiber"));
    assert_eq!(
        files.removed(),
        vec![PathBuf::from("/opt/lemonfiber/.lemonfiber-can-write")],
        "the probe is taken away whatever it came to"
    );
}

/// And one that will takes the probe away again, so a run leaves nothing behind.
#[tokio::test]
async fn a_probe_that_succeeded_is_taken_away_again() {
    let files = Program::ordinary().shared();
    let http = Fake::always(Answer::reply(200, released("v0.99.0")));

    let report = standing(&files, &http, Some("/opt/lemonfiber/lemonfiber")).await;

    assert_eq!(report.replaceable, Some(true));
    assert_eq!(
        files.removed(),
        vec![PathBuf::from("/opt/lemonfiber/.lemonfiber-can-write")]
    );
}

/// Nothing about this machine travels. The address requires a name of anybody asking
/// and that name is the same word in every copy of this program, which is the whole
/// of what leaves.
#[tokio::test]
async fn the_request_carries_nothing_that_would_tell_one_installation_from_another() {
    let files = Program::ordinary().shared();
    let http = Fake::always(Answer::reply(200, released("v0.99.0")));

    standing(&files, &http, Some("/opt/lemonfiber/lemonfiber")).await;

    let sent = http.requests();
    assert_eq!(sent.len(), 1, "{sent:?}");
    let Some(asked) = sent.first() else {
        unreachable!("one request was made");
    };
    assert!(asked.url.starts_with(RELEASE_LIST), "{}", asked.url);
    assert_eq!(asked.body, None);
    let carried = asked
        .headers
        .iter()
        .map(|(name, value)| format!("{name}: {value}"))
        .collect::<Vec<String>>()
        .join(" | ");
    assert!(!carried.contains(env!("CARGO_PKG_VERSION")), "{carried}");
    assert!(!carried.contains(HOME), "{carried}");
    assert!(!carried.contains("lemonfiber/"), "{carried}");
}

/// The switch reaches the code that would make the request, which is the half that
/// makes it mean anything: a refused check is one that never opened the connection.
#[tokio::test]
async fn a_check_the_operator_switched_off_never_reaches_the_release_list() {
    let files = Program::ordinary().shared();
    let http = Fake::silent();
    let ctx = ctx(
        &files,
        &http,
        Settings {
            reaching: Reaching::without(REACH_UPDATES_KEY),
            ..settings(Some("/opt/lemonfiber/lemonfiber"))
        },
    );

    let report = asked(Command::SelfUpdate { to: None }, &ctx).await;

    assert!(http.requests().is_empty(), "{:?}", http.requests());
    assert_eq!(report.standing, Standing::CheckFailed);
    let untold = report.untold.unwrap_or_default();
    assert!(untold.contains("settings say so"), "{untold}");
    assert!(files.written().is_empty(), "{:?}", files.written());
}

/// And the blanket switch is the same thing said once. An operator who wants nothing
/// to leave this machine has not made an exception for this.
#[tokio::test]
async fn an_operator_who_wants_nothing_to_leave_is_not_made_an_exception_of() {
    let files = Program::ordinary().shared();
    let http = Fake::silent();
    let ctx = ctx(
        &files,
        &http,
        Settings {
            reaching: Reaching::none(),
            ..settings(Some("/opt/lemonfiber/lemonfiber"))
        },
    );

    asked(Command::SelfUpdate { to: None }, &ctx).await;

    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

/// Running the same read twice reaches the network once. What was read is remembered,
/// and a version that came out this morning is no more useful to know about now than
/// in an hour.
#[tokio::test]
async fn a_check_made_today_is_answered_from_what_it_read_rather_than_asked_again() {
    let files = Program::ordinary()
        .holding(record(), &remembered(Some(NOW), Some("0.99.0"), 0))
        .shared();
    let http = Fake::silent();

    let report = standing(&files, &http, Some("/opt/lemonfiber/lemonfiber")).await;

    assert!(http.requests().is_empty(), "{:?}", http.requests());
    assert_eq!(report.offered.as_deref(), Some("0.99.0"));
    assert_eq!(report.standing, Standing::UpdateAvailable);
    assert_eq!(report.untold, None);
}

/// A day later it asks again, which is what makes the answer above a delay rather
/// than a memory that never refreshes.
#[tokio::test]
async fn a_day_later_the_release_list_is_asked_again() {
    let files = Program::ordinary()
        .holding(record(), &remembered(Some(NOW - A_DAY), Some("0.12.0"), 0))
        .shared();
    let http = Fake::always(Answer::reply(200, released("v0.99.0")));

    let report = standing(&files, &http, Some("/opt/lemonfiber/lemonfiber")).await;

    assert_eq!(http.requests().len(), 1);
    assert_eq!(report.offered.as_deref(), Some("0.99.0"));
}

/// A record that has never been answered and is not due yet says which of the four
/// reasons that is, rather than reading as a machine with no route out.
#[tokio::test]
async fn a_check_that_is_not_due_and_has_read_nothing_says_that_rather_than_a_fault() {
    let files = Program::ordinary()
        .holding(record(), &remembered(Some(NOW), None, 0))
        .shared();
    let http = Fake::silent();

    let report = standing(&files, &http, Some("/opt/lemonfiber/lemonfiber")).await;

    assert!(http.requests().is_empty(), "{:?}", http.requests());
    let untold = report.untold.unwrap_or_default();
    assert!(untold.contains("not been asked yet today"), "{untold}");
}

/// Enough failures in a row and it stops entirely. A laptop that has been off the
/// network for a week must not still be reaching for it on every run.
#[tokio::test]
async fn a_machine_that_has_given_up_asking_is_not_made_to_ask_again() {
    let files = Program::ordinary()
        .holding(record(), &remembered(Some(NOW - A_DAY * 100), None, 5))
        .shared();
    let http = Fake::silent();

    let report = standing(&files, &http, Some("/opt/lemonfiber/lemonfiber")).await;

    assert!(http.requests().is_empty(), "{:?}", http.requests());
    let untold = report.untold.unwrap_or_default();
    assert!(untold.contains("stopped asking"), "{untold}");
    assert!(untold.contains("starts again"), "{untold}");
}

/// An address that says nothing is written down as a failure, so the next attempt is
/// further off than this one was.
#[tokio::test]
async fn an_address_that_says_nothing_is_written_down_and_reported_as_that() {
    let files = Program::ordinary().shared();
    let http = Fake::silent();

    let report = standing(&files, &http, Some("/opt/lemonfiber/lemonfiber")).await;

    assert_eq!(report.standing, Standing::CheckFailed);
    let untold = report.untold.unwrap_or_default();
    assert!(untold.contains("did not answer"), "{untold}");
    let written = files.written();
    assert_eq!(written.len(), 1, "{written:?}");
    let (at, held) = written.first().cloned().unwrap_or_default();
    assert_eq!(at, record());
    assert!(held.contains(r#""quiet":1"#), "{held}");
}

/// A status that is not a success has answered, and it has answered with nothing a
/// version can be read out of — which to a caller is the same thing as silence.
#[tokio::test]
async fn an_address_that_refuses_is_the_same_as_one_that_says_nothing() {
    let files = Program::ordinary().shared();
    let http = Fake::always(Answer::reply(403, "rate limited"));

    let report = standing(&files, &http, Some("/opt/lemonfiber/lemonfiber")).await;

    assert_eq!(report.standing, Standing::CheckFailed);
    let written = files.written();
    let (_, held) = written.first().cloned().unwrap_or_default();
    assert!(held.contains(r#""quiet":1"#), "{held}");
}

/// An answer that reached the address and held no version this can order counts as
/// answered — nothing failed — and is recorded as such rather than as a failure.
#[tokio::test]
async fn an_answer_holding_no_orderable_version_is_not_recorded_as_a_failure() {
    let files = Program::ordinary().shared();
    let http = Fake::always(Answer::reply(200, released("nightly")));

    let report = standing(&files, &http, Some("/opt/lemonfiber/lemonfiber")).await;

    assert_eq!(report.offered, None);
    let written = files.written();
    let (_, held) = written.first().cloned().unwrap_or_default();
    assert!(held.contains(r#""quiet":0"#), "{held}");
}

/// A record this run cannot make sense of reads as a machine that has never asked,
/// and asking is what a machine that has never asked does.
#[tokio::test]
async fn a_record_that_will_not_read_is_a_machine_that_has_never_asked() {
    let files = Program::ordinary()
        .holding(record(), "this is not a record")
        .shared();
    let http = Fake::always(Answer::reply(200, released("v0.99.0")));

    let report = standing(&files, &http, Some("/opt/lemonfiber/lemonfiber")).await;

    assert_eq!(http.requests().len(), 1);
    assert_eq!(report.offered.as_deref(), Some("0.99.0"));
}

/// A machine with nowhere to keep a record still asks. What it loses is the quiet,
/// not the answer, and that is the right way round: a run that refused to check
/// because it could not find a directory would be a check that had become a
/// precondition.
#[tokio::test]
async fn a_machine_with_nowhere_to_keep_a_record_still_asks_and_writes_nothing() {
    let files = Program::ordinary().shared();
    let http = Fake::always(Answer::reply(200, released("v0.99.0")));
    let ctx = ctx(
        &files,
        &http,
        Settings {
            env_file: None,
            ..settings(Some("/opt/lemonfiber/lemonfiber"))
        },
    );

    let report = asked(Command::SelfUpdate { to: None }, &ctx).await;

    assert_eq!(http.requests().len(), 1);
    assert_eq!(report.offered.as_deref(), Some("0.99.0"));
    assert!(files.written().is_empty(), "{:?}", files.written());
}

/// Nothing newer is current, whoever owns the file. A copy a package manager put
/// here and that is already the newest has nothing for that manager to do.
#[tokio::test]
async fn a_copy_with_nothing_newer_is_current_whoever_put_it_here() {
    let files = Program::ordinary().shared();
    let http = Fake::always(Answer::reply(200, released("v0.0.1")));

    let report = standing(
        &files,
        &http,
        Some("/opt/homebrew/Cellar/lemonfiber/0.13.0/bin/lemonfiber"),
    )
    .await;

    assert_eq!(report.standing, Standing::Current);
    assert_eq!(report.offered.as_deref(), Some("0.0.1"));
}

/// A copy that is already the newest is offered nothing to type. The command for the
/// version it is running is an instruction to reinstall, and it reads as a next step
/// to whoever was looking for one.
#[tokio::test]
async fn a_copy_that_is_already_the_newest_is_offered_nothing_to_type() {
    let files = Program::ordinary().shared();
    let http = Fake::always(Answer::reply(200, released("v0.0.1")));

    let report = standing(&files, &http, Some("/opt/lemonfiber/lemonfiber")).await;

    assert_eq!(report.standing, Standing::Current);
    assert_eq!(report.command, None);
    assert_eq!(report.instead, None);
}

/// Naming a version is a different question, and it is answered whatever this copy
/// already is — otherwise there would be no way to ask for an older one from the
/// newest release.
#[tokio::test]
async fn naming_a_version_is_answered_even_where_this_copy_is_the_newest() {
    let files = Program::ordinary().shared();
    let http = Fake::always(Answer::reply(200, released("v0.0.1")));
    let ctx = ctx(&files, &http, settings(Some(IN_CARGOS_BIN)));

    let report = asked(
        Command::SelfUpdate {
            to: Some("0.9.0".to_owned()),
        },
        &ctx,
    )
    .await;

    assert_eq!(report.standing, Standing::Current);
    let typed = report.command.unwrap_or_default();
    assert!(typed.contains("releases/download/v0.9.0/"), "{typed}");
}

/// Going back is asked for by naming the version, and is answered with the command
/// and with whether that version reads what is already on this machine.
#[tokio::test]
async fn going_back_is_answered_with_the_command_and_what_that_version_reads() {
    let files = Program::ordinary().shared();
    let http = Fake::always(Answer::reply(200, released("v0.99.0")));
    let ctx = ctx(&files, &http, settings(Some(IN_CARGOS_BIN)));

    let report = asked(
        Command::SelfUpdate {
            to: Some("0.9.0".to_owned()),
        },
        &ctx,
    )
    .await;

    assert_eq!(report.asked.as_deref(), Some("0.9.0"));
    let typed = report.command.unwrap_or_default();
    assert!(typed.contains("releases/download/v0.9.0/"), "{typed}");
    let reads = report.configuration.unwrap_or_default();
    assert!(
        reads.contains("0.9.0 is behind the copy running"),
        "{reads}"
    );
}

/// A manager that keeps an index cannot be told a version its index may not carry,
/// and naming one to it is refused with the reason rather than answered with a
/// different version.
#[tokio::test]
async fn a_manager_that_keeps_an_index_says_it_has_no_form_for_a_named_version() {
    let files = Program::ordinary().shared();
    let http = Fake::always(Answer::reply(200, released("v0.99.0")));
    let ctx = ctx(
        &files,
        &http,
        settings(Some(
            "/opt/homebrew/Cellar/lemonfiber/0.13.0/bin/lemonfiber",
        )),
    );

    let report = asked(
        Command::SelfUpdate {
            to: Some("0.9.0".to_owned()),
        },
        &ctx,
    )
    .await;

    assert_eq!(report.command, None);
    let instead = report.instead.unwrap_or_default();
    assert!(instead.contains("brew uninstall"), "{instead}");
}

/// What updating leaves alone is said every time rather than when asked, and so is
/// what a release brings besides the program.
#[tokio::test]
async fn what_updating_leaves_alone_and_what_it_brings_are_said_every_time() {
    let files = Program::ordinary().shared();
    let http = Fake::silent();

    let report = standing(&files, &http, None).await;

    assert!(
        report
            .afterwards
            .contains("Nothing in the stack is stopped"),
        "{}",
        report.afterwards
    );
    assert!(
        report.carries.contains("manifest schema"),
        "{}",
        report.carries
    );
    assert!(
        report.carries.contains("newer service images"),
        "{}",
        report.carries
    );
}

/// The property the whole family rests on: this cannot refuse. Every seam is against
/// it at once — nowhere to look, nothing answering, nothing readable — and the
/// answer is still an answer.
#[tokio::test]
async fn nothing_here_can_refuse_however_little_could_be_told() {
    let files = Program::ordinary().unresolvable().unwritable().shared();
    let http = Fake::silent();
    let ctx = ctx(&files, &http, Settings::default());

    let answered = dispatch(Command::SelfUpdate { to: None }, &ctx).await;

    assert!(answered.is_ok(), "{answered:?}");
    assert_eq!(
        answered.ok().map(|outcome| outcome.envelope().kind),
        Some(lemonfiber_core::model::kind::SELF_UPDATE)
    );
}
