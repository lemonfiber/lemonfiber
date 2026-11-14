use super::proving::{beneath, verdicts};
use super::*;

/// A container that will not start stops the install by name, and the files it had
/// already written go back.
#[tokio::test]
async fn a_service_that_will_not_start_stops_the_install_and_the_files_go_back() {
    let runner = Keyed::answering(
        vec![("up", Ok(engine_refused("no such image")))],
        Ok(spoke("")),
    );
    let ctx = proving("unstartable", runner, Fake::silent());
    let at = source("unstartable", PROVING);

    let (code, said) = refused(installing(&ctx, &at).await);
    assert_eq!(code, "PLUGIN-9");
    assert!(
        said.contains("was put back") && !said.contains("Still standing"),
        "a reversal that finished says so and names nothing as standing: {said}"
    );
    assert!(
        made_paths(&ctx)
            .iter()
            .all(|path| !std::path::Path::new(path).exists()),
        "every path this run made is gone"
    );
}

/// A reversal that could not take the container off says what is still standing,
/// rather than repeating the sentence a reversal that finished would have got.
#[tokio::test]
async fn a_service_that_will_not_start_and_will_not_come_off_names_what_stands() {
    let runner = Keyed::answering(
        vec![
            ("up", Ok(engine_refused("no such image"))),
            ("rm", Ok(engine_refused("no such container"))),
        ],
        Ok(spoke("")),
    );
    let ctx = proving("standing", runner, Fake::silent());

    let (code, said) = refused(installing(&ctx, &source("standing", PROVING)).await);
    assert_eq!(code, "PLUGIN-9");
    assert!(
        said.contains("Still standing: komga"),
        "the container nothing could take off is named: {said}"
    );
}

/// A container engine that is not there at all is a different answer from one that
/// ran and refused, and both stop the install — the engine's own words underneath
/// the install's account rather than in place of it.
#[tokio::test]
async fn an_engine_that_cannot_be_run_at_all_stops_the_install() {
    let runner = Arc::new(Recording::answering(Err(
        lemonfiber_ports::process::Failure::NotFound {
            program: "docker".to_owned(),
        },
    )));
    let ctx = proving("no-engine", runner, Fake::silent());

    assert_eq!(
        beneath(installing(&ctx, &source("no-engine", PROVING)).await),
        ("PLUGIN-9".to_owned(), "PROC-1".to_owned()),
        "the install's own account leads, and what the engine said is carried \
             underneath it rather than dropped"
    );
}

/// A proof naming a method the transport cannot send is never put, and says so.
/// Sending a different verb than the one written down would be proving something
/// nobody declared.
#[tokio::test]
async fn a_proof_naming_a_method_lemonfiber_cannot_send_is_never_put() {
    let traced = PROVING.replace("method = \"GET\"", "method = \"TRACE\"");
    let ctx = proving(
        "traced",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    );

    let stated = verdicts(installing(&ctx, &source("traced", &traced)).await);
    assert_eq!(came_to(stated.first().and_then(Option::as_ref)), "unproven");
    assert!(why(stated.first().and_then(Option::as_ref)).contains("TRACE"));
}

/// A container the engine would not take off the machine is named as still
/// standing, because *some of it worked* is the sentence that sends somebody
/// looking by hand.
#[tokio::test]
async fn a_container_that_would_not_come_off_is_named_as_still_standing() {
    let runner = Keyed::answering(
        vec![("rm", Ok(engine_refused("no such container")))],
        Ok(spoke("")),
    );
    let ctx = proving("stuck", runner.clone(), answering(503));

    let left = report(installing(&ctx, &source("stuck", PROVING)).await)
        .and_then(|one| one.install)
        .and_then(|one| one.reversed)
        .map(|back| back.left)
        .unwrap_or_default();
    assert_eq!(left.len(), 1);
    assert!(left
        .first()
        .is_some_and(|one| one.because.contains("could not be taken off")));
    assert!(
        runner.ran("rm"),
        "and it was asked, which is what makes the refusal the engine's rather than \
             an account of a call nobody made"
    );
}

/// An install that found its directory already there puts back only what it wrote: the
/// directory belongs to whoever made it, and stays.
#[tokio::test]
async fn an_install_over_a_directory_already_there_puts_back_only_what_it_wrote() {
    let ctx = proving(
        "leftovers",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(503),
    );
    let existing = stack_of(&ctx).join("config/komga");
    assert!(std::fs::create_dir_all(&existing).is_ok());

    let put_back = report(installing(&ctx, &source("leftovers", PROVING)).await)
        .and_then(|one| one.install)
        .and_then(|one| one.reversed)
        .unwrap_or_default();

    assert!(put_back
        .reversed
        .iter()
        .all(|undo| Path::new(&undo.target) != existing));
    assert!(put_back.left.is_empty());
    assert!(existing.is_dir());
    assert!(!project_of(&ctx).exists());
}
