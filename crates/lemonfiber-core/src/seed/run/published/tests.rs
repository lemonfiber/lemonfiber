use super::published_as;

/// The name is the service's own, upper-cased.
#[test]
fn a_key_is_published_under_the_service_it_came_from() {
    assert_eq!(published_as("sonarr"), "SONARR_API_KEY");
    assert_eq!(published_as("sabnzbd"), "SABNZBD_API_KEY");
}

/// A hyphen is not something an environment name may carry.
///
/// Service ids are Compose names and several of them are hyphenated. A shell
/// reads a hyphen as an operator, so a name built straight from the id would be
/// one nothing could reference.
#[test]
fn a_hyphenated_service_is_published_under_a_name_a_shell_can_read() {
    assert_eq!(
        published_as("calibre-web-automated"),
        "CALIBRE_WEB_AUTOMATED_API_KEY"
    );
}

/// A machine holding [`crate::test_support::a_trusted_crossing_reader`], its file holding
/// a stale key, answering where that file is.
fn reading(name: &str) -> (crate::app::Ctx, std::path::PathBuf) {
    let env = crate::test_support::env_without_password(name);
    let mut register = crate::plugin::Register::empty();
    assert!(register
        .record(crate::test_support::a_trusted_crossing_reader())
        .is_ok());
    assert!(std::fs::write(
        env.with_file_name(crate::config::paths::PLUGINS),
        serde_json::to_string(&register).unwrap_or_default()
    )
    .is_ok());
    let ctx = crate::test_support::a_context()
        .settings(crate::config::Settings {
            env_file: Some(env.clone()),
            ..crate::config::Settings::default()
        })
        .build();
    let file = env.with_file_name("env").join("reader.env");
    let _ = std::fs::create_dir_all(env.with_file_name("env"));
    let _ = std::fs::write(&file, "SONARR_KEY=a-stale-key\n");
    (ctx, file)
}

/// What setup publishes: the series curator's key, as `key`.
fn published(key: &str) -> Vec<(String, String)> {
    vec![("SONARR_API_KEY".to_owned(), key.to_owned())]
}

/// A key recorded anew is written into every secrets file that reads it there and then,
/// so nothing that reads it keeps the one it replaced; a key recorded as it already was
/// writes nothing.
#[test]
fn a_curator_key_recorded_anew_reaches_every_file_that_reads_it() {
    let (ctx, file) = reading("published-anew");

    assert_eq!(
        super::recorded(&ctx, published("the-new-key")),
        crate::seed::State::Wired
    );
    assert!(std::fs::read_to_string(&file).is_ok_and(|text| text.contains("the-new-key")));

    let _ = std::fs::write(&file, "SONARR_KEY=left-alone\n");
    assert_eq!(
        super::recorded(&ctx, published("the-new-key")),
        crate::seed::State::Wired
    );
    assert_eq!(
        std::fs::read_to_string(&file).ok().as_deref(),
        Some("SONARR_KEY=left-alone\n")
    );
}

/// A key recorded anew that a file reading it cannot take is said, apart from a key that
/// could not be recorded at all.
#[cfg(unix)]
#[test]
fn a_curator_key_recorded_anew_that_a_file_cannot_take_is_said() {
    use std::os::unix::fs::PermissionsExt as _;
    let (ctx, file) = reading("published-unwritten");
    let directory = file
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();
    let _ = std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o555));

    let state = super::recorded(&ctx, published("the-new-key"));

    let _ = std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755));
    assert!(
        matches!(&state, crate::seed::State::Failed { detail }
            if detail.starts_with("these keys were recorded and could not be written")
                && detail.contains("SONARR_API_KEY")),
        "{state:?}"
    );
}
