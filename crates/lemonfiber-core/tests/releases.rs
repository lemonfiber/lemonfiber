//! The releases check, driven through its public surface against fakes.
//!
//! The check opens a resolution service, asks for one wanted item, and searches for it,
//! reading the result against the profile. Both sides are faked here: a filesystem that
//! hands back the service's config, and a transport routed by URL so the wanted list and
//! the search answer independently. The check is built on `#[async_trait]` clients built
//! on another, so — as with the credentials check and the Servarr client — it is
//! exercised from here rather than a `#[cfg(test)]` module, where async-trait code is
//! compiled twice and its coverage counted from the wrong copy.

use std::path::PathBuf;
use std::sync::Arc;

use lemonfiber_core::doctor::credentials::Target;
use lemonfiber_core::doctor::releases::{ReleasesCheck, NONE_AVAILABLE, PRESET_UNMET};
use lemonfiber_core::doctor::{Category, Check, Verdict};
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::http::{Answer, Fake};

/// The Servarr config that opens a target, carrying a readable key.
const CONFIG_WITH_KEY: &str = "<Config><ApiKey>a1b2c3d4e5</ApiKey></Config>";

/// Where the Sonarr config is read from.
fn config_path() -> PathBuf {
    PathBuf::from("/stack/config/sonarr/config.xml")
}

fn sonarr() -> Target {
    Target {
        id: "sonarr".to_owned(),
        name: "Sonarr".to_owned(),
        base: "http://127.0.0.1:8989".to_owned(),
        config: config_path(),
        version: 3,
    }
}

/// A filesystem that opens the Sonarr target.
fn opening_fs() -> Arc<Files> {
    Files::at(vec![(config_path(), CONFIG_WITH_KEY)])
}

/// Run a disruptive check over one Sonarr target and return its single verdict.
async fn sonarr_verdict(fs: Arc<Files>, http: Arc<Fake>) -> Verdict {
    let check = ReleasesCheck::new(http, fs, vec![sonarr()], true);
    let mut findings = check.run().await;
    findings.pop().map_or(
        Verdict::Skipped {
            reason: String::new(),
        },
        |found| found.verdict,
    )
}

/// A finding about one service says which, so a failure here can be attributed to the
/// thing underneath it and quoted with what that service said for itself. Without the
/// name, both of those features simply skip this check.
#[tokio::test]
async fn a_finding_about_a_service_names_it() {
    let check = ReleasesCheck::new(Fake::silent(), opening_fs(), vec![sonarr()], true);

    let named: Vec<Option<String>> = check
        .run()
        .await
        .into_iter()
        .map(|finding| finding.service)
        .collect();

    assert!(
        !named.is_empty()
            && named
                .iter()
                .all(|service| service.as_deref() == Some("sonarr")),
        "every finding this check makes is about the service it searched, and it made at \
         least one: {named:?}"
    );
}

/// A wanted list naming one missing episode.
const ONE_WANTED: &str = r#"{"records":[{"id":42}]}"#;

/// The code a warning verdict carries, or `None` if it was not a warning — so a test
/// asserts on the code without a panicking match arm.
fn warned(verdict: &Verdict) -> Option<lemonfiber_core::error::Code> {
    match verdict {
        Verdict::Warn(problem) => Some(problem.code),
        _ => None,
    }
}

#[tokio::test]
async fn releases_meeting_the_preset_pass() {
    // Wanted content is found, and the profile would grab it — the preset is met.
    let http = Fake::by_path(vec![
        ("wanted/missing", Answer::reply(200, ONE_WANTED)),
        ("release", Answer::reply(200, r#"[{"rejections":[]}]"#)),
    ]);
    assert!(matches!(
        sonarr_verdict(opening_fs(), http).await,
        Verdict::Pass { .. }
    ));
}

#[tokio::test]
async fn releases_the_preset_rejects_warn_that_the_preset_is_unmet() {
    // Releases exist, but the profile wants none of them — the preset conflicts with
    // what is available, distinct from an indexer failure.
    let http = Fake::by_path(vec![
        ("wanted/missing", Answer::reply(200, ONE_WANTED)),
        (
            "release",
            Answer::reply(
                200,
                r#"[{"rejections":["Quality Bluray-2160p is not wanted in profile"]}]"#,
            ),
        ),
    ]);
    let verdict = sonarr_verdict(opening_fs(), http).await;
    assert_eq!(warned(&verdict), Some(PRESET_UNMET));
}

#[tokio::test]
async fn a_clean_search_that_finds_nothing_warns_that_none_are_available() {
    let http = Fake::by_path(vec![
        ("wanted/missing", Answer::reply(200, ONE_WANTED)),
        ("release", Answer::reply(200, "[]")),
    ]);
    let verdict = sonarr_verdict(opening_fs(), http).await;
    assert_eq!(warned(&verdict), Some(NONE_AVAILABLE));
}

#[tokio::test]
async fn a_search_that_cannot_run_is_unverified_not_a_verdict_on_the_preset() {
    // The wanted list answered, but the search itself did not — the indexer-failure
    // case: nothing was learned about releases, so it is unverified, never a warning.
    let http = Fake::by_path(vec![("wanted/missing", Answer::reply(200, ONE_WANTED))]);
    assert!(matches!(
        sonarr_verdict(opening_fs(), http).await,
        Verdict::Unverified { .. }
    ));
}

#[tokio::test]
async fn nothing_wanted_is_skipped_rather_than_searched() {
    let http = Fake::by_path(vec![(
        "wanted/missing",
        Answer::reply(200, r#"{"records":[]}"#),
    )]);
    assert!(matches!(
        sonarr_verdict(opening_fs(), http).await,
        Verdict::Skipped { .. }
    ));
}

#[tokio::test]
async fn a_service_that_has_not_started_is_skipped() {
    // No config on disk, so the target does not open and there is nothing to search.
    let http = Fake::by_path(vec![("wanted/missing", Answer::reply(200, ONE_WANTED))]);
    let fs = Files::at(Vec::new());
    assert!(matches!(
        sonarr_verdict(fs, http).await,
        Verdict::Skipped { .. }
    ));
}

#[tokio::test]
async fn no_resolution_service_is_skipped() {
    // A stack with no television or film service has no releases to check.
    let http = Fake::by_path(Vec::new());
    let check = ReleasesCheck::new(http, opening_fs(), Vec::new(), true);
    let verdict = check.run().await.pop().map(|found| found.verdict);
    assert!(matches!(verdict, Some(Verdict::Skipped { .. })));
}

#[tokio::test]
async fn an_ordinary_run_skips_the_live_search() {
    // Not disruptive: the check reports skipped rather than querying the indexer.
    let http = Fake::by_path(Vec::new());
    let check = ReleasesCheck::new(http, opening_fs(), vec![sonarr()], false);
    let findings = check.run().await;
    assert_eq!(findings.len(), 1);
    assert!(matches!(
        findings.first().map(|found| &found.verdict),
        Some(Verdict::Skipped { .. })
    ));
}

#[tokio::test]
async fn a_film_service_is_searched_by_its_own_id() {
    // Radarr searches by movieId rather than episodeId — the same probe, the other
    // resolution service, so both media types' search parameters are exercised.
    let http = Fake::by_path(vec![
        ("wanted/missing", Answer::reply(200, ONE_WANTED)),
        ("release", Answer::reply(200, r#"[{"rejections":[]}]"#)),
    ]);
    let radarr = Target {
        id: "radarr".to_owned(),
        name: "Radarr".to_owned(),
        base: "http://127.0.0.1:7878".to_owned(),
        config: PathBuf::from("/stack/config/radarr/config.xml"),
        version: 3,
    };
    let fs = Files::at(vec![(
        PathBuf::from("/stack/config/radarr/config.xml"),
        CONFIG_WITH_KEY,
    )]);
    let verdict = ReleasesCheck::new(http, fs, vec![radarr], true)
        .run()
        .await
        .pop()
        .map(|found| found.verdict);
    assert!(matches!(verdict, Some(Verdict::Pass { .. })));
}

#[tokio::test]
async fn the_check_is_a_services_check() {
    let check = ReleasesCheck::new(Fake::by_path(Vec::new()), opening_fs(), Vec::new(), true);
    assert_eq!(check.category(), Category::Services);
}
