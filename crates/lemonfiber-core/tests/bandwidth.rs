//! Reading what the line carries and declaring a limit on it, through the
//! dispatcher.
//!
//! From here rather than from a `#[cfg(test)]` module for the reason the forms
//! listing is: the app layer is compiled twice, and a path exercised only in-crate
//! has its coverage counted from the copy that never ran. What that copy holds is
//! the arm joining the command to its handler — the only thing standing between a
//! surface asking about the line and the code that answers.
//!
//! Nothing is faked and nothing is reached. With no environment file configured
//! there is no recorded password to open the torrent client with and no key written
//! for the Usenet one, so both are left out rather than dialled; there is nowhere to
//! keep a record either, so the run writes nothing. What is left is exactly the
//! question this file is about: does asking reach the answering.

mod common;

use common::stack::project;
use std::path::Path;
use std::sync::Arc;

use lemonfiber_core::adapters::{Daemon, Disk, Local, System};
use lemonfiber_core::app::{dispatch, BandwidthAsked, Command, Ctx, Outcome};
use lemonfiber_core::bandwidth::{Restraint, NOTHING_TO_LIMIT};
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::stack::{Source, STACK_UNREADABLE};

fn ctx(stack: Source) -> Ctx {
    Ctx::new(
        Arc::new(Local),
        Arc::new(Daemon::local()),
        Arc::new(System),
        Arc::new(Disk),
        stack,
        Settings::default(),
        Environment::MacOs,
    )
}

/// A request naming one thing.
fn asking(field: impl FnOnce(&mut BandwidthAsked)) -> BandwidthAsked {
    let mut asked = BandwidthAsked::default();
    field(&mut asked);
    asked
}

/// Asked for nothing, the command reads and reports rather than writing anything.
#[tokio::test]
async fn a_request_that_names_no_limit_reads_the_line_and_writes_nothing() {
    let read = dispatch(
        Command::Bandwidth(BandwidthAsked::default()),
        &ctx(Source::External(project())),
    )
    .await;

    assert!(
        matches!(&read, Ok(Outcome::Bandwidth(shared))
            if !shared.applied
                && shared.restraint == Restraint::Unlimited
                && !shared.untouched.is_empty()),
        "nothing was put to a client, and what is never limited is still said: {read:?}"
    );
}

/// The report leaves this crate under the kind a parser selects it by.
///
/// Serialised from here as well as in-crate: the envelope arm is a line of whichever
/// copy does the serialising, and a surface reading `--json` reads this one.
#[tokio::test]
async fn the_line_is_reported_under_the_kind_a_parser_reads_it_by() {
    let json = dispatch(
        Command::Bandwidth(BandwidthAsked::default()),
        &ctx(Source::External(project())),
    )
    .await
    .ok()
    .and_then(|outcome| outcome.envelope().to_json())
    .unwrap_or_default();

    assert!(json.contains(r#""kind":"bandwidth""#), "{json}");
    assert!(json.contains(r#""applied":false"#), "{json}");
}

/// A limit needs somewhere to go, and this stack has no client to put one in.
///
/// Refused rather than recorded: a declaration kept where nothing enforces it is a
/// household believing its evening is protected while the stack takes the whole line.
#[tokio::test]
async fn a_limit_with_no_download_client_to_hold_is_refused_rather_than_recorded() {
    let refused = dispatch(
        Command::Bandwidth(asking(|asked| asked.down = Some("2MiB".to_owned()))),
        &ctx(Source::External(project())),
    )
    .await;

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(NOTHING_TO_LIMIT)
    );
}

/// A stack that will not read is the operator's own `--stack-dir`, and they hear
/// that rather than a line reported as having no clients on it.
#[tokio::test]
async fn a_stack_that_cannot_be_read_is_said_rather_than_read_as_a_line_with_no_clients() {
    let refused = dispatch(
        Command::Bandwidth(BandwidthAsked::default()),
        &ctx(Source::External(Path::new("/lemonfiber/no/such/stack"))),
    )
    .await;

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(STACK_UNREADABLE),
        "the two read alike from out here, and only one of them is the operator's to fix"
    );
}
