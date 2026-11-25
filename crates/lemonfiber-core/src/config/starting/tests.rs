use std::path::PathBuf;
use std::time::Duration;

use lemonfiber_fixtures::scratch::Scratch;

use super::{Settings, Surroundings};
use crate::config::paths::Paths;
use crate::config::{
    Resending, EXPLANATIONS_KEY, IDEMPOTENCY_KEYS_KEY, IDEMPOTENCY_MINUTES_KEY, PROJECT_KEY,
};
use crate::platform::{engine_is_desktop, Environment, HOST_OS};
use crate::ports::docker::Target;
use crate::test_support::a_context;

const ID: &str = "4f1c0d7e9a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f708192a3b4c5";

fn kept(named: &str, env: &str) -> (Scratch, Paths) {
    let scratch = Scratch::named(&format!("starting-{named}"));
    let dir = scratch.to_path_buf();
    let _ = std::fs::remove_dir_all(&dir);
    let paths = Paths::at(&dir.join("config"), &dir.join("data"));
    let _ = std::fs::create_dir_all(paths.config_dir());
    let _ = std::fs::write(paths.env_file(), env);
    (scratch, paths)
}

fn around(paths: &Paths) -> Surroundings {
    Surroundings {
        paths: Some(paths.clone()),
        program: Some(PathBuf::from("/usr/local/bin/lemonfiber")),
        home: Some(PathBuf::from("/home/media")),
        docker: Target::local(),
        host_locale: Some("nl_NL.UTF-8".to_owned()),
        mounts: Some(format!(
            "541 512 8:1 /var/lib/docker/containers/{ID}/hostname /etc/hostname rw - ext4 /dev/sda1 rw\n"
        )),
    }
}

#[test]
fn a_run_starts_from_its_env_file_and_what_only_its_surface_knows() {
    let (_held, paths) = kept(
        "read",
        &format!("{PROJECT_KEY}=media\n{EXPLANATIONS_KEY}=off\n"),
    );

    let read = Settings::read(None, &around(&paths));

    assert_eq!(read.project, "media");
    assert!(!read.explanations);
    assert!(!read.autostart_on_battery);
    assert_eq!(read.env_file, Some(paths.env_file()));
    assert_eq!(read.stack_dir, Some(paths.stack()));
    assert_eq!(read.admission, Some(paths.admission()));
    assert_eq!(read.container.as_deref(), Some(ID));
    assert_eq!(read.host_locale.as_deref(), Some("nl_NL.UTF-8"));
    assert_eq!(read.home, Some(PathBuf::from("/home/media")));
    assert_eq!(
        read.program,
        Some(PathBuf::from("/usr/local/bin/lemonfiber"))
    );
}

#[test]
fn a_run_with_nowhere_of_its_own_starts_from_nothing_recorded() {
    let read = Settings::read(None, &Surroundings::default());

    assert_eq!(read.env_file, None);
    assert_eq!(read.container, None);
    assert!(read.explanations);
    assert_eq!(read.resending(), Resending::default());
}

#[tokio::test]
async fn a_started_run_reads_its_settings_again_through_its_own_records() {
    let (_held, paths) = kept("again", &format!("{PROJECT_KEY}=media\n"));
    let surroundings = around(&paths);
    let ctx = a_context()
        .settings(Settings::read(None, &surroundings))
        .build();

    ctx.filed().await;
    let again = Settings::read(Some(&ctx), &surroundings);
    let written = Settings::as_written(&ctx, &paths.env_file());

    assert_eq!(again.project, "media");
    assert_eq!(written.data_root, again.data_root);
    assert_eq!(written.project, ctx.settings.project);
}

#[test]
fn how_a_request_sent_again_is_answered_is_read_from_the_env_file() {
    let (_held, paths) = kept(
        "resending",
        &format!("{IDEMPOTENCY_MINUTES_KEY}=5\n{IDEMPOTENCY_KEYS_KEY}=3\n"),
    );

    let bounds = Settings::read(None, &around(&paths)).resending();

    assert_eq!(bounds.within, Duration::from_secs(300));
    assert_eq!(bounds.at_most, 3);
}

#[test]
fn this_machine_is_named_from_its_host_and_the_engine_it_reaches() {
    let local = Settings::default();

    assert_eq!(
        local.environment(),
        Environment::resolve(HOST_OS, engine_is_desktop(&local.docker))
    );
    assert_eq!(local.settle_host(), None);
}
