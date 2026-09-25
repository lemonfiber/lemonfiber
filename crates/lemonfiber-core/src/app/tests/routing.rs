//! Each command reaching the handler that answers it.

use super::*;

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
        .and_then(|outcome| outcome.envelope().to_json())
        .unwrap_or_default();
    // The envelope is what this is about: the wire version, the kind, and the
    // report nested under `data` rather than spread beside it. What the report
    // itself holds is its own tests' business — the changelog of every release
    // this project has cut is in there now, and a literal of it here would be a
    // test that has to be rewritten every time one is tagged.
    assert!(
        rendered.starts_with(concat!(
            r#"{"api_version":1,"kind":"version","data":{"binary":""#,
            env!("CARGO_PKG_VERSION"),
            r#"","supported_schema":[1],"stack":"0.1.0","compose":"v2.32.1","#
        )),
        "{rendered}"
    );
    assert!(rendered.ends_with("}}"), "{rendered}");
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
        Some(crate::error::codes::stack::STACK_UNREADABLE),
        "an operator's own --stack-dir mistake reaches them"
    );
}

/// Asking about the line arrives at the command that answers about it.
///
/// Asking what you are told about reaches the command that answers it.
///
/// Dispatched here as well as from `tests/`: the arm is a line of each copy of this
/// file, and the copy that never dispatched it counts the arm as never run.
#[tokio::test]
async fn asking_what_is_already_here_reaches_the_command_that_surveys_it() {
    let ctx = a_context().build();
    let read = dispatch(Command::Migrate(MigrateAction::Survey), &ctx).await;
    let answered = matches!(&read, Ok(Outcome::Migration(_)));
    assert!(answered, "{read:?}");
}

#[tokio::test]
async fn asking_to_carry_records_across_reaches_the_command_that_would() {
    let ctx = a_context().build();
    let asked = Command::Migrate(MigrateAction::Act {
        mode: Mode::Import,
        confirmed: false,
    });
    let read = dispatch(asked, &ctx).await;
    let answered = matches!(&read, Ok(Outcome::Import(_)));
    assert!(answered, "{read:?}");
}

#[tokio::test]
async fn asking_to_stand_in_place_of_what_is_here_reaches_the_command_that_would() {
    let ctx = a_context().build();
    let asked = Command::Migrate(MigrateAction::Act {
        mode: Mode::Replace,
        confirmed: false,
    });
    let read = dispatch(asked, &ctx).await;
    let answered = matches!(&read, Ok(Outcome::Replacement(_)));
    assert!(answered, "{read:?}");
}

#[tokio::test]
async fn asking_to_stand_beside_what_is_here_reaches_the_command_that_would() {
    let ctx = a_context().build();
    let asked = Command::Migrate(MigrateAction::Act {
        mode: Mode::Beside,
        confirmed: false,
    });
    let read = dispatch(asked, &ctx).await;
    let answered = matches!(&read, Ok(Outcome::Beside(_)));
    assert!(answered, "{read:?}");
}

#[tokio::test]
async fn asking_to_take_over_what_is_here_reaches_the_command_that_would() {
    let ctx = a_context().build();
    let asked = Command::Migrate(MigrateAction::Act {
        mode: Mode::Adopt,
        confirmed: false,
    });
    let read = dispatch(asked, &ctx).await;
    let answered = matches!(&read, Ok(Outcome::Adoption(_)));
    assert!(answered, "{read:?}");
}

#[tokio::test]
async fn asking_what_you_are_told_about_reaches_the_command_that_reads_it() {
    let ctx = a_context().build();
    let read = dispatch(Command::Alerts(AlertAction::Show), &ctx).await;
    let answered = matches!(&read, Ok(Outcome::Alerts(_)));
    assert!(answered, "{read:?}");
}

/// Reading what the stack holds reaches the command that answers it.
///
/// Dispatched here as well as from `tests/` for the same reason as the line
/// below: the arm is a line of each copy of this file, and the copy that never
/// dispatched it counts the arm as never run.
#[tokio::test]
async fn asking_about_the_credentials_reaches_the_command_that_reads_them() {
    let ctx = a_context().build();
    let read = dispatch(Command::Credentials(Asking::Read), &ctx).await;
    let answered = matches!(&read, Ok(Outcome::Credentials(_)));
    assert!(answered, "{read:?}");
}

/// Surveying a removal reaches the command that lists it.
///
/// The listing is the read half of an uninstall and takes nothing away, so it is
/// the one that can be dispatched here without a stack to remove.
#[tokio::test]
async fn surveying_a_removal_reaches_the_command_that_lists_it() {
    let ctx = a_context().build();
    let listed = dispatch(
        Command::Uninstall(Removing::surveying(crate::uninstall::Tier::Stop)),
        &ctx,
    )
    .await;
    let reached = matches!(&listed, Ok(Outcome::Uninstall(_)));
    assert!(reached, "{listed:?}");
}

/// Dispatched here as well as from `tests/`: this file is compiled twice, and
/// the arm joining a command to its handler is a line of each copy — so the
/// copy that never dispatched it counts the arm as never run. What the command
/// does is settled beside the command itself; this is about arriving there.
#[tokio::test]
async fn asking_about_the_line_reaches_the_command_that_reads_it() {
    let ctx = a_context().build();
    let read = dispatch(Command::Bandwidth(BandwidthAsked::default()), &ctx).await;
    // Bound rather than asserted inline: a multi-line `matches!` inside an
    // assertion that carries a message leaves the condition's own line counted as
    // never run, which the coverage gate reads as dead code.
    let wrote_nothing = matches!(&read, Ok(Outcome::Bandwidth(shared)) if !shared.applied);
    assert!(
        wrote_nothing,
        "a run that asked for nothing wrote nothing: {read:?}"
    );
}

/// The start a login makes reaches the run that decides whether to make it, and a
/// rehearsal of it is permitted rather than refused.
///
/// Dispatched here as well as from `tests/`, and for the reason the two above are:
/// this file is compiled twice, and the arm joining a command to its handler is a
/// line of each copy — so the copy that never dispatched it counts the arm as never
/// run. Both ways round, because the table saying what a rehearsal of a command
/// means is read only on a rehearsal, and it lives in a second file compiled twice
/// over as well.
///
/// A machine nobody has answered the autostart question on declines, which is the
/// cheapest of the four answers in front of the start and the only one reachable
/// without an engine, a stack of containers, or a machine that has actually
/// restarted. What the other three come to is settled beside the run itself; this
/// is about arriving there.
#[tokio::test]
async fn the_start_a_login_makes_reaches_the_run_that_decides_whether_to_make_it() {
    for ctx in [a_context().build(), a_context().build().rehearsing()] {
        let read = dispatch(Command::AtBoot, &ctx).await;
        let declined = matches!(&read, Ok(Outcome::Lifecycle(report)) if report.held.is_some());
        assert!(declined, "{read:?}");
    }
}
