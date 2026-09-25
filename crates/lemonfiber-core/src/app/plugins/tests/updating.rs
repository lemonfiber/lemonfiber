//! Replacing one version of a plugin with another, or putting the old one back.

use super::*;

/// The whole of an update that holds: the new version is on, and the record names it.
/// The installed version's container came off before the new one went on, and the
/// document the stack reads is the new version's.
#[tokio::test]
async fn an_update_that_holds_leaves_the_machine_on_the_new_version() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("update-held", runner.clone(), answering(200));
    assert_eq!(
        counted(installing(&ctx, &source("update-held", PROVING)).await),
        Some(1)
    );
    assert!(document(&ctx).contains(PINNED));

    let done = update(updating(&ctx, &source("update-held-next", &next())).await);

    assert_eq!(done.as_ref().map(|one| one.install.recorded), Some(true));
    assert_eq!(
        done.as_ref()
            .map(|one| (one.from.as_str(), one.to.as_str())),
        Some(("1.2.0", "1.3.0"))
    );
    assert!(done.as_ref().is_some_and(|one| one.restored.is_none()));
    assert_eq!(
        on(&ctx).await.as_deref(),
        Some("1.3.0"),
        "the record names the new one"
    );
    let now = document(&ctx);
    assert!(now.contains(REPINNED), "and the stack reads it: {now}");
    assert!(runner.ran("rm") && runner.ran("up"));
}

/// A rehearsal gives the whole account — what would go back, what would come on,
/// what it would prove and what would stop — and touches none of it.
#[tokio::test]
async fn a_rehearsed_update_is_one_account_and_touches_nothing() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let mut ctx = proving("update-rehearsed", runner.clone(), answering(200));
    assert_eq!(
        counted(installing(&ctx, &source("update-rehearsed", PROVING)).await),
        Some(1)
    );
    ctx.dry_run = true;
    let before = runner.seen().len();

    let would = update(updating(&ctx, &source("update-rehearsed-next", &next())).await);

    assert!(would.as_ref().is_some_and(|one| one.went_back.rehearsed
        && !one.went_back.reversed.is_empty()
        && !one.install.changes.is_empty()
        && !one.install.proofs.is_empty()
        && one.interrupts == ["komga"]
        && !one.install.recorded
        && one.restored.is_none()));
    assert_eq!(
        runner.seen().len(),
        before,
        "nothing was asked of the engine"
    );
    assert_eq!(on(&ctx).await.as_deref(), Some("1.2.0"));
    assert!(document(&ctx).contains(PINNED), "and nothing moved");
}

/// A new version whose proof does not hold goes back, and the one it replaced is put
/// back on from its record: the machine is on the old version, not between the two,
/// and the report says so.
#[tokio::test]
async fn an_update_whose_proof_does_not_hold_leaves_the_machine_on_the_old_version() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let mut ctx = proving("update-unproved", runner, answering(200));
    assert_eq!(
        counted(installing(&ctx, &source("update-unproved", PROVING)).await),
        Some(1)
    );
    ctx = ctx.with_http(answering(503));

    let failed = update(updating(&ctx, &source("update-unproved-next", &next())).await);

    assert!(failed.as_ref().is_some_and(|one| !one.install.recorded
        && one.install.reversed.is_some()
        && one.stopped.is_none()));
    assert_eq!(
        failed.and_then(|one| one.restored),
        Some(crate::plugin::Restored {
            version: "1.2.0".to_owned(),
            placed: true,
            running: true,
        })
    );
    assert_eq!(
        on(&ctx).await.as_deref(),
        Some("1.2.0"),
        "the record never moved"
    );
    let now = document(&ctx);
    assert!(
        now.contains(PINNED) && !now.contains(REPINNED),
        "and the stack reads the old version again: {now}"
    );
}

/// A new version that will not start is stopped before its proofs are asked, and
/// says why. Here the engine refuses every start, so the old one's files come back
/// and its container does not — which the report says, rather than calling it back.
#[tokio::test]
async fn an_update_that_will_not_start_says_what_the_old_version_came_back_as() {
    let mut ctx = proving(
        "update-unstarted",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    );
    assert_eq!(
        counted(installing(&ctx, &source("update-unstarted", PROVING)).await),
        Some(1)
    );
    ctx.seams.runner = Keyed::answering(vec![("up", Ok(engine_refused("no")))], Ok(spoke("")));

    let failed = update(updating(&ctx, &source("update-unstarted-next", &next())).await);

    assert!(failed
        .as_ref()
        .and_then(|one| one.stopped.as_deref())
        .is_some_and(|why| why.contains("refused to start")));
    assert_eq!(
        failed
            .and_then(|one| one.restored)
            .map(|back| (back.placed, back.running)),
        Some((true, false)),
        "its files are back and its container is not running"
    );
    assert_eq!(on(&ctx).await.as_deref(), Some("1.2.0"));
}

/// A disk that refuses partway through the old version going back does not leave the
/// machine between the two: the new one is never put on, the old one is put back on,
/// and the report says what stopped it.
#[tokio::test]
async fn an_update_whose_old_version_will_not_come_fully_off_puts_it_back() {
    use std::os::unix::fs::PermissionsExt as _;

    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("update-held-fast", runner, answering(200));
    assert_eq!(
        counted(installing(&ctx, &source("update-held-fast", PROVING)).await),
        Some(1)
    );
    // The directory the plugin's own configuration directory sits in, made unwritable,
    // so the one entry an empty directory can still be refused on is refused.
    let config = stack_of(&ctx).join("config");
    let locked = std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o500));

    let failed = update(updating(&ctx, &source("update-held-fast-next", &next())).await);

    let _ = std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o700));
    assert!(locked.is_ok());
    assert!(failed.as_ref().is_some_and(|one| !one.install.recorded
        && one.install.reversed.is_none()
        && one
            .stopped
            .as_deref()
            .is_some_and(|why| why.contains("could not be taken fully off"))));
    assert_eq!(
        failed
            .and_then(|one| one.restored)
            .map(|back| (back.placed, back.running)),
        Some((true, true))
    );
    assert_eq!(on(&ctx).await.as_deref(), Some("1.2.0"));
    assert!(
        document(&ctx).contains(PINNED),
        "the old version's document is back"
    );
}

/// Where the installed version's containers will not come off, nothing else is
/// touched: the refusal says the old version is still installed, and it is.
#[tokio::test]
async fn an_update_whose_old_version_will_not_stop_changes_nothing() {
    let mut ctx = proving(
        "update-stuck",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    );
    assert_eq!(
        counted(installing(&ctx, &source("update-stuck", PROVING)).await),
        Some(1)
    );
    ctx.seams.runner = Keyed::answering(vec![("rm", Ok(engine_refused("no")))], Ok(spoke("")));

    let (code, said) = refused(updating(&ctx, &source("update-stuck-next", &next())).await);

    assert_eq!(code, "PLUGIN-12");
    assert!(said.contains("still installed"), "{said}");
    assert_eq!(on(&ctx).await.as_deref(), Some("1.2.0"));
    assert!(
        document(&ctx).contains(PINNED),
        "its files are where they were"
    );
}

/// An update needs a version to replace, and a plugin with none is refused naming
/// the verb that would do what was meant.
#[tokio::test]
async fn updating_what_is_not_installed_is_refused_naming_the_install() {
    let ctx = proving(
        "update-nothing",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    );
    let (code, _) = refused(updating(&ctx, &source("update-nothing", &next())).await);
    assert_eq!(code, "PLUGIN-11");
    assert!(!record_of(&ctx).exists());
}

/// The rollback layer's refusal comes before anything is taken, as a removal's does:
/// an update that heard it after stopping the container would leave the version the
/// record names with nothing of it running.
#[tokio::test]
async fn a_drifted_setting_refuses_the_update_before_anything_is_taken() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("update-drifted", runner.clone(), answering(200));
    assert_eq!(
        counted(installing(&ctx, &source("update-drifted", PROVING)).await),
        Some(1)
    );
    let key = "LEMONFIBER_PLUGIN_TEST_KEY";
    journal_a_set(&ctx, "komga", key, "what the plugin wrote");
    let _ = ctx
        .settings
        .env_file
        .as_deref()
        .map(|file| crate::config::store::set(file, key, "what the operator wrote"));

    let (code, _) = refused(updating(&ctx, &source("update-drifted-next", &next())).await);

    assert_eq!(code, "UNDO-3");
    assert!(!runner.ran("rm"), "nothing was taken");
    assert_eq!(on(&ctx).await.as_deref(), Some("1.2.0"));
}

/// The next version, with a second service beside the first.
fn next_with_a_second_service() -> String {
    next().replace(
        "\n[[proof]]",
        r#"
[[service]]
id          = "komga-stats"
name        = "Komga statistics"
image       = "example.invalid/komga-stats"
digest      = "sha256:6f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.0.0"
port        = 25601
bind        = "loopback"
criticality = "enhancing"
config_path = "/app/data"

[[proof]]
service = "komga""#,
    )
}

/// A write the new version cannot make stops it before it is started, says which,
/// and the old version is put back on.
#[tokio::test]
async fn an_update_whose_new_version_will_not_land_puts_the_old_one_back() {
    let ctx = proving(
        "update-unwritten",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    );
    assert_eq!(
        counted(installing(&ctx, &source("update-unwritten", PROVING)).await),
        Some(1)
    );
    // Where the new service's configuration directory would go, a file of somebody
    // else's — which the new version must not remove and cannot write through.
    let blocking = stack_of(&ctx).join("config/komga-stats");
    assert!(std::fs::write(&blocking, "somebody else's").is_ok());

    let failed = update(
        updating(
            &ctx,
            &source("update-unwritten-next", &next_with_a_second_service()),
        )
        .await,
    );

    assert!(failed.as_ref().is_some_and(|one| one.stopped.is_some()
        && one.install.reversed.is_some()
        && !one.install.recorded));
    assert_eq!(
        failed
            .and_then(|one| one.restored)
            .map(|back| (back.placed, back.running)),
        Some((true, true))
    );
    assert_eq!(on(&ctx).await.as_deref(), Some("1.2.0"));
    assert!(blocking.is_file(), "and what was in the way is untouched");
}

/// An engine that cannot be run at all stops the new version too, and the reason
/// is the engine's own words.
#[tokio::test]
async fn an_update_with_no_engine_to_start_it_says_so() {
    let mut ctx = proving(
        "update-no-engine",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    );
    assert_eq!(
        counted(installing(&ctx, &source("update-no-engine", PROVING)).await),
        Some(1)
    );
    ctx.seams.runner = Keyed::answering(
        vec![(
            "up",
            Err(lemonfiber_ports::process::Failure::NotFound {
                program: "docker".to_owned(),
            }),
        )],
        Ok(spoke("")),
    );

    let failed = update(updating(&ctx, &source("update-no-engine-next", &next())).await);

    assert!(failed
        .and_then(|one| one.stopped)
        .is_some_and(|why| why.contains("`docker` was not found")));
}

/// A new version that holds its own proofs and breaks the stack's checks does not
/// hold, for the reason an install that did would not: what it broke is said in its
/// own account, and the version it replaced is put back on.
#[tokio::test]
async fn an_update_that_breaks_the_stack_puts_the_old_version_back() {
    let mut ctx = proving(
        "update-collateral",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    );
    assert_eq!(
        counted(installing(&ctx, &source("update-collateral", PROVING)).await),
        Some(1)
    );
    ctx.seams.runner = LostToTheInstall::losing("version");

    let failed = update(updating(&ctx, &source("update-collateral-next", &next())).await);

    assert!(failed.as_ref().is_some_and(|one| one.stopped.is_none()
        && one
            .install
            .verified
            .as_ref()
            .is_some_and(|checked| !checked.held())));
    assert!(failed.is_some_and(|one| one.restored.is_some()));
    assert_eq!(on(&ctx).await.as_deref(), Some("1.2.0"));
}

/// A record that cannot be rewritten stops an update that otherwise held, and the
/// version it replaced is put back on rather than left under a record that names
/// it with the new one running.
#[tokio::test]
async fn an_update_whose_record_cannot_be_written_puts_the_old_version_back() {
    let ctx = proving(
        "update-unrecordable",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    );
    assert_eq!(
        counted(installing(&ctx, &source("update-unrecordable", PROVING)).await),
        Some(1)
    );
    let register = record_of(&ctx);
    let locked = unrewritable(&register);

    let failed = update(updating(&ctx, &source("update-unrecordable-next", &next())).await);

    let _ = std::fs::remove_dir_all(staging_of(&register));
    assert!(locked);
    assert!(failed.as_ref().is_some_and(|one| one
        .stopped
        .as_deref()
        .is_some_and(|why| why.contains("record of what is installed could not be written"))));
    assert!(failed.is_some_and(|one| one.restored.is_some()));
    assert_eq!(on(&ctx).await.as_deref(), Some("1.2.0"));
}

/// The refusals an update gives before anything moves, each its own: a source this
/// build cannot read, a machine with no stack, and a stack that cannot be read.
#[tokio::test]
async fn an_update_refuses_before_anything_moves_for_what_it_cannot_read() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("update-refusals", runner.clone(), answering(200));
    assert_eq!(
        counted(installing(&ctx, &source("update-refusals", PROVING)).await),
        Some(1)
    );
    let before = runner.seen().len();

    let empty = lemonfiber_fixtures::scratch::Scratch::named("update-empty").kept();
    let _ = std::fs::create_dir_all(&empty);
    assert_eq!(refusal(updating(&ctx, &empty).await), "PLUGIN-2");

    let blind = a_context()
        .over(crate::test_support::nowhere())
        .settings(ctx.settings.clone())
        .build();
    assert_eq!(
        refusal(updating(&blind, &source("update-refusals-next", &next())).await),
        crate::error::codes::stack::STACK_UNREADABLE.to_string()
    );

    let mut stackless = ctx;
    stackless.settings.stack_dir = None;
    assert_eq!(
        refusal(updating(&stackless, &source("update-refusals-next", &next())).await),
        "PLUGIN-6"
    );
    assert_eq!(
        runner.seen().len(),
        before,
        "nothing was asked of the engine"
    );
    assert_eq!(on(&stackless).await.as_deref(), Some("1.2.0"));
}

/// A rehearsal is refused for drift exactly as the real run is, because it asks the
/// same judgement.
#[tokio::test]
async fn a_rehearsed_update_is_refused_for_drift_as_the_real_one_is() {
    let mut ctx = proving(
        "update-drifted-rehearsed",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    );
    assert_eq!(
        counted(installing(&ctx, &source("update-drifted-rehearsed", PROVING)).await),
        Some(1)
    );
    let key = "LEMONFIBER_PLUGIN_TEST_KEY";
    journal_a_set(&ctx, "komga", key, "what the plugin wrote");
    let _ = ctx
        .settings
        .env_file
        .as_deref()
        .map(|file| crate::config::store::set(file, key, "what the operator wrote"));
    ctx.dry_run = true;

    assert_eq!(
        refusal(updating(&ctx, &source("update-drifted-rehearsed-next", &next())).await),
        "UNDO-3"
    );
}
