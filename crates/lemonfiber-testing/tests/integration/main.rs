//! The context builder, each part of it reaching the context it builds.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::docker::{Engine, Images};
use lemonfiber_core::ports::process::Runner;
use lemonfiber_core::ports::{Clock, FileSystem};
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::ports::{Idle, Stopped};
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::support::Reporting;
use lemonfiber_testing::{a_context, a_live_context, nowhere};

#[test]
fn each_seam_a_test_names_is_the_one_the_run_reaches() {
    let runner: Arc<dyn Runner> = Arc::new(Idle);
    let engine: Arc<dyn Engine> = Arc::new(Reporting::absent());
    let clock: Arc<dyn Clock> = Stopped::at(5);
    let files: Arc<dyn FileSystem> = Files::empty();
    let pulled: Arc<dyn Images> = Pulled::holding(Vec::new());

    let ctx = a_context()
        .runner(Arc::clone(&runner))
        .engine(Arc::clone(&engine))
        .clock(Arc::clone(&clock))
        .filesystem(Arc::clone(&files))
        .images(Arc::clone(&pulled))
        .build();

    assert!(Arc::ptr_eq(&ctx.seams.runner, &runner));
    assert!(Arc::ptr_eq(&ctx.seams.engine, &engine));
    assert!(Arc::ptr_eq(&ctx.seams.filesystem, &files));
    assert!(Arc::ptr_eq(&ctx.seams.images, &pulled));
    assert_eq!(ctx.seams.clock.now(), UNIX_EPOCH + Duration::from_secs(5));
}

#[test]
fn the_stack_settings_and_platform_a_test_names_are_the_ones_the_run_holds() {
    let settings = Settings {
        stack_dir: Some("/srv/stack".into()),
        ..Settings::default()
    };

    let ctx = a_context()
        .over(nowhere())
        .settings(settings)
        .environment(Environment::LinuxNative)
        .build();

    assert!(
        ctx.stack.manifest().is_err(),
        "a stack pointing nowhere was read as one"
    );
    assert_eq!(
        ctx.settings.stack_dir.as_deref(),
        Some(Path::new("/srv/stack"))
    );
    assert_eq!(ctx.environment, Environment::LinuxNative);
}

#[test]
fn a_live_context_reads_the_clock_on_the_wall() {
    let wall = SystemTime::now();
    let read = a_live_context().build().seams.clock.now();

    let apart = read
        .duration_since(wall)
        .unwrap_or_else(|behind| behind.duration());
    assert!(
        apart < Duration::from_secs(60),
        "{apart:?} from the wall clock"
    );
}
