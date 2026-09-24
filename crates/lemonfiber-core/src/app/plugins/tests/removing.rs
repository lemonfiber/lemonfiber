//! Taking a plugin off, and what goes with it.

use super::*;

/// A rehearsed removal on a machine with no stack is refused by the layer that looks
/// for the record, as the real one is.
#[tokio::test]
async fn a_rehearsed_removal_with_nowhere_to_look_is_refused_as_the_real_one_is() {
    let ctx = proving(
        "no-stack-rehearsed",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    );
    assert_eq!(
        counted(installing(&ctx, &source("no-stack-rehearsed", PROVING)).await),
        Some(1)
    );
    let mut nowhere = ctx;
    nowhere.settings.stack_dir = None;
    nowhere.dry_run = true;
    assert_eq!(refusal(removing(&nowhere, "komga").await), "UNDO-4");
}

/// A disk that refuses partway through a removal's reversal is reported, and the
/// plugin stays on the record — the next removal is what finishes it, rather than a
/// record that says it is gone with its files still on the disk.
#[tokio::test]
async fn a_removal_the_disk_refuses_partway_leaves_the_plugin_recorded() {
    use std::os::unix::fs::PermissionsExt as _;

    let ctx = proving(
        "removal-refused-partway",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    );
    assert_eq!(
        counted(installing(&ctx, &source("removal-refused-partway", PROVING)).await),
        Some(1)
    );
    let config = stack_of(&ctx).join("config");
    let locked = std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o500));

    let stopped = refusal(removing(&ctx, "komga").await);

    let _ = std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o700));
    assert!(locked.is_ok());
    assert_eq!(stopped, crate::app::recover::NOT_REMOVED.to_string());
    assert_eq!(
        counted(reading(&ctx).await),
        Some(1),
        "it is still recorded"
    );
}

/// A narrator that notes, as it hears each line, whether the container had already
/// been taken off by then — which is the whole of what *before* means here.
struct Heard {
    runner: Arc<Recording>,
    said: std::sync::Mutex<Vec<(String, bool)>>,
}

#[async_trait::async_trait]
impl crate::ports::Narrator for Heard {
    async fn say(&self, said: &str) {
        let already = self.runner.ran("rm");
        let _ = self
            .said
            .lock()
            .map(|mut heard| heard.push((said.to_owned(), already)));
    }
}

/// What stops is named before anything does, and named by service. The report of a
/// real removal arrives after the containers are gone, so a removal that only put it
/// there would be telling an operator about something already past.
#[tokio::test]
async fn what_a_removal_stops_is_said_before_it_stops_it() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let mut ctx = proving("interrupting", runner.clone(), answering(200));
    assert_eq!(
        counted(installing(&ctx, &source("interrupting", PROVING)).await),
        Some(1)
    );
    let heard = Arc::new(Heard {
        runner: runner.clone(),
        said: std::sync::Mutex::new(Vec::new()),
    });
    ctx.narrator = heard.clone();

    let gone = removal(removing(&ctx, "komga").await);

    assert_eq!(
        gone.map(|one| one.interrupts),
        Some(vec!["komga".to_owned()]),
        "the report names the service it took away"
    );
    let said = heard
        .said
        .lock()
        .map(|heard| heard.clone())
        .unwrap_or_default();
    assert_eq!(
        said,
        vec![(
            "removing komga stops komga — for good, since nothing is left to start again"
                .to_owned(),
            false
        )],
        "said once, and while the container was still there"
    );
    assert!(runner.ran("rm"), "and then it was taken off");
}

/// A rehearsal names the same services in its report and says nothing aloud: it
/// stops nothing, and the report is read before anybody agrees to the real run.
#[tokio::test]
async fn a_rehearsed_removal_names_what_it_would_stop_and_says_nothing_aloud() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let mut ctx = proving("interrupting-rehearsed", runner.clone(), answering(200));
    assert_eq!(
        counted(installing(&ctx, &source("interrupting-rehearsed", PROVING)).await),
        Some(1)
    );
    let heard = Arc::new(Heard {
        runner,
        said: std::sync::Mutex::new(Vec::new()),
    });
    ctx.narrator = heard.clone();
    ctx.dry_run = true;

    let would = removal(removing(&ctx, "komga").await);

    assert_eq!(
        would.map(|one| one.interrupts),
        Some(vec!["komga".to_owned()])
    );
    assert_eq!(
        heard.said.lock().map_or(1, |heard| heard.len()),
        0,
        "nothing is said aloud about a stop that is not happening"
    );
}

/// Taking one of two off leaves the record holding the other, rather than being
/// taken away with it.
#[tokio::test]
async fn removing_one_of_two_leaves_the_record_holding_the_other() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("two-of-them", runner, answering(200));
    assert_eq!(
        counted(installing(&ctx, &source("two-of-them", PROVING)).await),
        Some(1)
    );
    let second = PROVING.replace("\"komga\"", "\"kavita\"");
    assert_eq!(
        counted(installing(&ctx, &source("two-of-them-again", &second)).await),
        Some(2)
    );

    let gone = removal(removing(&ctx, "komga").await);
    assert!(gone.as_ref().is_some_and(|one| one.removed));
    assert!(
        gone.is_some_and(|one| one.went_back.left.iter().any(|left| left
            .because
            .contains("still holds something this run did not put there"))),
        "the directory the two share is named and left, rather than taken with the \
             other's document in it or stopping the reversal over it"
    );
    assert_eq!(
        counted(reading(&ctx).await),
        Some(1),
        "the other is still there"
    );
    assert!(
        record_of(&ctx).exists(),
        "and the record is kept rather than taken away"
    );
}

/// A stack this build cannot read stops a removal before it takes anything,
/// because what the machine would be left without cannot be answered without it —
/// and answering *nothing* would be a guess with a removal attached to it.
#[tokio::test]
async fn a_removal_on_an_unreadable_stack_is_refused_before_it_takes_anything() {
    let ctx = proving(
        "unreadable-stack",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    );
    assert_eq!(
        counted(installing(&ctx, &source("unreadable-stack", PROVING)).await),
        Some(1)
    );
    let document = stack_of(&ctx).join("compose/plugins/komga.yml");

    let blind = a_context()
        .over(crate::test_support::nowhere())
        .settings(ctx.settings.clone())
        .build();
    assert_eq!(
        refusal(removing(&blind, "komga").await),
        crate::stack::STACK_UNREADABLE.to_string()
    );
    assert!(document.is_file(), "and nothing of its was taken");
}

/// A rehearsal says what would go back and touches none of it. A removal an
/// operator has not agreed to yet is a reading, and a reading that removed a
/// container would be the write done to describe itself.
#[tokio::test]
async fn rehearsing_a_removal_says_what_would_go_back_and_takes_nothing() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("rehearsed-removal", runner.clone(), answering(200));
    assert_eq!(
        counted(installing(&ctx, &source("rehearsed-removal", PROVING)).await),
        Some(1)
    );
    let document = stack_of(&ctx).join("compose/plugins/komga.yml");

    let mut rehearsing = ctx;
    rehearsing.dry_run = true;
    let gone = removal(removing(&rehearsing, "komga").await);

    assert_eq!(gone.as_ref().map(|one| one.removed), Some(false));
    assert!(
        gone.is_some_and(|one| one.went_back.rehearsed && !one.went_back.reversed.is_empty()),
        "it names what would go back"
    );
    assert!(document.is_file(), "and none of it went");
    assert!(
        !runner.ran("rm"),
        "nothing was taken off the machine either"
    );
    assert_eq!(counted(reading(&rehearsing).await), Some(1));
}

/// A name nothing is installed under is refused, and the refusal says what is —
/// because the commonest reason to reach it is a name spelled the way an operator
/// remembers it rather than the way the plugin declares it.
#[tokio::test]
async fn removing_something_that_is_not_installed_is_refused_naming_what_is() {
    let ctx = proving(
        "not-installed",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    );
    assert_eq!(
        refusal(removing(&ctx, "komga").await),
        "PLUGIN-10",
        "on a machine with nothing installed"
    );
    assert_eq!(
        counted(installing(&ctx, &source("not-installed", PROVING)).await),
        Some(1)
    );
    let (code, said) = refused(removing(&ctx, "komgaa").await);
    assert_eq!(code, "PLUGIN-10");
    assert!(
        said.contains("What is installed: komga"),
        "it names what is there: {said}"
    );
}

/// A container the engine would not take off is named as still standing, because
/// *some of it worked* is the sentence that sends somebody looking by hand — and
/// the exit status says so rather than reporting a removal that worked.
#[tokio::test]
async fn a_removal_whose_container_would_not_come_off_names_it_as_still_standing() {
    let runner = Keyed::answering(
        vec![("rm", Ok(engine_refused("no such container")))],
        Ok(spoke("")),
    );
    let ctx = proving("stuck-removal", runner, answering(200));
    assert_eq!(
        counted(installing(&ctx, &source("stuck-removal", PROVING)).await),
        Some(1)
    );

    let gone = removal(removing(&ctx, "komga").await);
    assert!(
        gone.as_ref().is_some_and(|one| one
            .went_back
            .left
            .iter()
            .any(|left| left.target == "komga" && left.because.contains("could not be taken off"))),
        "the container is named with the reason it is still there"
    );
    assert!(
        gone.is_some_and(|one| one.removed),
        "and the record is written all the same: the files went back, so a plugin \
             the register still named would be a plugin nothing describes"
    );
}

/// A machine with nowhere to look for what was changed is refused by the rollback
/// layer, in the rollback layer's own words. A removal that answered anyway would
/// be reporting a plugin as gone on the strength of a record it never read.
#[tokio::test]
async fn a_removal_with_nowhere_to_look_for_the_record_is_refused_by_the_layer_that_looks() {
    let ctx = proving(
        "no-stack",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    );
    assert_eq!(
        counted(installing(&ctx, &source("no-stack", PROVING)).await),
        Some(1)
    );

    let mut nowhere = ctx;
    nowhere.settings.stack_dir = None;

    assert_eq!(
        refusal(removing(&nowhere, "komga").await),
        "UNDO-4",
        "the layer that looks for the record is the one that says it cannot"
    );
}

/// A register that cannot be rewritten stops a removal the way it stops an
/// install, and says what the run left.
#[cfg(unix)]
#[tokio::test]
async fn a_register_that_cannot_be_rewritten_stops_the_removal() {
    use std::os::unix::fs::PermissionsExt as _;

    let ctx = proving(
        "unwritable-removal",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    );
    assert_eq!(
        counted(installing(&ctx, &source("unwritable-removal", PROVING)).await),
        Some(1)
    );
    let register = record_of(&ctx);
    assert!(std::fs::set_permissions(&register, std::fs::Permissions::from_mode(0o400)).is_ok());

    let (code, said) = refused(removing(&ctx, "komga").await);
    assert_eq!(code, "PLUGIN-8");
    assert!(said.contains("was put back"), "{said}");

    let _ = std::fs::set_permissions(&register, std::fs::Permissions::from_mode(0o600));
}

/// **A removal inherits the rollback layer's refusals rather than restating them.**
/// A setting somebody has set by hand since is drift, and putting it back would
/// discard their edit — so the removal refuses and says what the file holds instead
/// of overwriting it.
#[tokio::test]
async fn a_setting_edited_since_the_install_refuses_the_removal() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("drifted", runner.clone(), answering(200));
    assert_eq!(
        counted(installing(&ctx, &source("drifted", PROVING)).await),
        Some(1)
    );

    // A setting the plugin is on the record as having written, and an operator's
    // own value in the file where that change would be put back.
    let key = "LEMONFIBER_PLUGIN_TEST_KEY";
    journal_a_set(&ctx, "komga", key, "what the plugin wrote");
    let _ = ctx
        .settings
        .env_file
        .as_deref()
        .map(|file| crate::config::store::set(file, key, "what the operator wrote"));

    let (code, said) = refused(removing(&ctx, "komga").await);
    assert_eq!(
        code, "UNDO-3",
        "the rollback layer's own refusal, not a second one"
    );
    assert!(
        said.contains("somebody has set it since"),
        "and its own words: {said}"
    );
    assert!(
        !runner.ran("rm"),
        "and refused before anything was taken: a removal that stopped the container \
             and then refused would leave a plugin recorded as installed and not running"
    );
    assert_eq!(
        counted(reading(&ctx).await),
        Some(1),
        "it is still installed"
    );
}

/// And the one refusal that is not a refusal: a change that re-points where data
/// lives goes back and says plainly that the data does not move with it.
#[tokio::test]
async fn re_pointing_where_data_lives_says_the_data_does_not_move_back() {
    let ctx = proving(
        "repointed",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    );
    assert_eq!(
        counted(installing(&ctx, &source("repointed", PROVING)).await),
        Some(1)
    );
    journal_a_set(
        &ctx,
        "komga",
        crate::config::DATA_ROOT_KEY,
        "/srv/elsewhere",
    );

    let gone = removal(removing(&ctx, "komga").await);
    assert!(
        gone.as_ref().is_some_and(|one| one.removed),
        "it is removed rather than refused"
    );
    assert!(
        gone.is_some_and(|one| one
            .went_back
            .noted
            .iter()
            .any(|note| note.because.contains("data does not move with it"))),
        "and the reversal says plainly that the library stays where it was moved to"
    );
}

/// **What a plugin contributed goes with it, and the run afterwards reads as one on
/// a machine that never saw it.** Compared whole rather than checked for an absent
/// row, because *answers exactly as* is a claim about the report and not about the
/// absence of one line in it.
///
/// Nothing withdraws anything. The rows a diagnosis runs are read from the register
/// on every run, so a plugin taken out of the register takes its rows with it —
/// which is why there is no withdrawal to get wrong.
#[tokio::test]
async fn what_a_plugin_contributed_goes_with_it_when_the_plugin_does() {
    let ctx = proving(
        "withdrawing",
        Arc::new(Recording::answering(Ok(spoke("")))),
        Fake::by_path(vec![
            (
                "/api/v1/libraries",
                lemonfiber_fixtures::http::Answer::reply(200, "[]"),
            ),
            (
                "/api/v1/claim",
                lemonfiber_fixtures::http::Answer::reply(200, "{}"),
            ),
        ]),
    );
    // Every finding but the one that counts lemonfiber's own files. A machine that
    // has installed and removed a plugin has a history of having done so, and the
    // journal holding it is one of the files whose permissions are checked. That is
    // a true fact about the machine and must not be erased: what the rule asks to go
    // is what the plugin contributed, not the record that it was here.
    let looked = || async {
        crate::app::diagnose(&ctx, &crate::doctor::Narrowing::Suite, false)
            .await
            .map(|report| {
                report
                    .findings
                    .into_iter()
                    .filter(|finding| finding.check != "config.credential-permissions")
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };

    let never = looked().await;
    assert!(!never.is_empty(), "the bundled rows ran");
    assert_eq!(
        counted(installing(&ctx, &source("withdrawing", CONTRIBUTING)).await),
        Some(1)
    );
    let holding = looked().await;
    assert_ne!(holding, never, "the plugin's row was there to be withdrawn");

    assert!(removal(removing(&ctx, "komga").await).is_some_and(|one| one.removed));
    assert_eq!(
        looked().await,
        never,
        "and a run after it reads as one on a machine that never saw it"
    );
}
