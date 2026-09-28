//! Handing each service's key and the *arrs to the services that read them.

use super::*;

/// The request service is handed the \*arrs, read from the \*arrs themselves.
///
/// Everything the request service needs to fetch through one — where it is, what
/// to authenticate with, which profile and which folder — comes off the \*arr
/// rather than being assumed here.
#[tokio::test]
async fn the_arrs_in_the_stack_are_handed_to_the_request_service() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let http = Fake::by_path_in_turn(vec![
        (
            "/qualityprofile",
            vec![Answer::reply(200, r#"[{"id":4,"name":"HD-1080p"}]"#)],
        ),
        (
            "/rootfolder",
            vec![Answer::reply(200, r#"[{"id":1,"path":"/data/media/tv"}]"#)],
        ),
        ("/settings/radarr", vec![Answer::reply(200, "[]")]),
        (
            "/settings/sonarr",
            vec![
                Answer::reply(200, "[]"),
                Answer::reply(201, ""),
                Answer::reply(200, r#"[{"id":1,"hostname":"sonarr","port":8989}]"#),
            ],
        ),
    ]);
    let ctx = seed_ctx(None, true, Vec::new(), None, None)
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)));

    let wirings = super::super::seed_fulfilment_targets(
        &ctx,
        &[arr("sonarr", 8989, "tv"), seerr_svc()],
        Some(std::path::Path::new("/opt/lemonfiber/stack")),
    )
    .await;

    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&crate::seed::State::Wired),
        "the *arr was not handed over: {wirings:?}"
    );
    let sent = http
        .requests()
        .into_iter()
        .find(|asked| asked.method == Method::Post)
        .and_then(|asked| asked.body)
        .unwrap_or_default();
    assert!(
        sent.contains("\"hostname\":\"sonarr\"") && sent.contains("HD-1080p"),
        "the request service was told where to reach it and what to fetch at: {sent}"
    );
}

/// Each service's key is published where the stack's own services read it.
///
/// Two of them are configured by the environment and by nothing else — there is no
/// endpoint to tell them anything — so a key that is read and not written down is one
/// the quality sync and the archive extractor both run without. Asserted on the file, because that is the whole of the mechanism.
#[tokio::test]
async fn each_services_key_is_published_where_the_stack_reads_it() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let env = recorded_admin("published");
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)));

    let wiring = super::super::published::publish_keys(
        &ctx,
        &[arr("sonarr", 8989, "tv")],
        Some(std::path::Path::new("/opt/lemonfiber/stack")),
        Some("sab-key"),
    )
    .await;

    assert_eq!(wiring.state, crate::seed::State::Wired, "{wiring:?}");
    let written = std::fs::read_to_string(&env).unwrap_or_default();
    assert!(
        written.contains("SONARR_API_KEY=the-key"),
        "the *arr's key was read and never written down: {written}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// The listening server, whose key is one lemonfiber makes an account for.
fn audiobookshelf_svc() -> lemonfiber_manifest::Service {
    manifest_service(
        "audiobookshelf",
        Some(lemonfiber_manifest::Api {
            kind: lemonfiber_manifest::ApiKind::Audiobookshelf,
            key_source: lemonfiber_manifest::KeySource::Generated,
            path: None,
            version: None,
        }),
        Some(13378),
    )
}

/// Every key a service wrote down is published, even where no \*arr list holds it.
///
/// The media-filing \*arrs are the right set for root folders and download clients and
/// the wrong one here: Prowlarr manages no media and so declares no media types, and
/// the subtitle finder is not Servarr-shaped at all, and both have a key. qBittorrent
/// is reached by name and password rather than by key, and half a credential
/// authenticates as badly as none.
///
/// **What nothing reads is retired on the way.** The media server's key
/// filed under lemonfiber's name is revoked and forgotten, and the listening server's
/// token is forgotten: nothing reads either, and each administers its whole service.
#[tokio::test]
async fn every_service_with_a_key_is_published_not_only_the_ones_that_file_media() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let env = recorded_admin("published-widely");
    // The name is published to pair with a password already minted, so record one.
    let _ = store::set(
        &env,
        crate::config::QBITTORRENT_PASSWORD_KEY,
        "minted-earlier",
    );
    // The media server signs in and lists its keys, one of them lemonfiber's; the
    // listening server says it already has an account. What an earlier run published
    // for the dashboard is in the file, to be retired.
    let _ = store::set(
        &env,
        crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        "minted-earlier",
    );
    let _ = store::set(
        &env,
        crate::config::AUDIOBOOKSHELF_PASSWORD_KEY,
        "minted-earlier",
    );
    let _ = store::set(&env, "JELLYFIN_API_KEY", "jellyfin-key");
    let _ = store::set(&env, "AUDIOBOOKSHELF_API_KEY", "listening-token");
    let http = Fake::by_path(vec![
        (
            "/Users/AuthenticateByName",
            Answer::reply(200, r#"{"AccessToken":"t"}"#),
        ),
        (
            "/Auth/Keys",
            Answer::reply(
                200,
                r#"{"Items":[{"AppName":"lemonfiber","AccessToken":"jellyfin-key"}]}"#,
            ),
        ),
        ("/status", Answer::reply(200, r#"{"isInit":true}"#)),
    ]);
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
        .with_http(http.clone())
        .with_filesystem(Arc::new(
            SeedFs::keyed(Some(KEYED), None)
                .with_bazarr(FINDER_CONFIG)
                .with_seerr(r#"{"main":{"apiKey":"request-key"}}"#),
        ));

    let wiring = super::super::published::publish_keys(
        &ctx,
        &[
            arr("sonarr", 8989, "tv"),
            prowlarr(),
            bazarr_svc(),
            seerr_with_settings(),
            jellyfin_svc(),
            audiobookshelf_svc(),
            manifest_service(
                "qbittorrent",
                Some(lemonfiber_manifest::Api {
                    kind: lemonfiber_manifest::ApiKind::Qbittorrent,
                    key_source: lemonfiber_manifest::KeySource::Generated,
                    path: None,
                    version: None,
                }),
                Some(8081),
            ),
        ],
        Some(std::path::Path::new("/opt/lemonfiber/stack")),
        None,
    )
    .await;

    assert_eq!(wiring.state, crate::seed::State::Wired, "{wiring:?}");
    let written = std::fs::read_to_string(&env).unwrap_or_default();
    for expected in [
        "SONARR_API_KEY=the-key",
        "PROWLARR_API_KEY=the-key",
        "BAZARR_API_KEY=finder-key",
        "SEERR_API_KEY=request-key",
        "QBITTORRENT_USERNAME=admin",
    ] {
        assert!(
            written.contains(expected),
            "{expected} was not published: {written}"
        );
    }
    for retired in ["JELLYFIN_API_KEY", "AUDIOBOOKSHELF_API_KEY"] {
        assert!(
            !written.contains(retired),
            "{retired} is still held for a dashboard that reads nothing: {written}"
        );
    }
    let revoked: Vec<String> = http
        .requests()
        .into_iter()
        .filter(|asked| asked.method == Method::Delete)
        .map(|asked| asked.url)
        .collect();
    assert_eq!(revoked.len(), 1, "{revoked:?}");
    assert!(
        revoked
            .iter()
            .all(|url| url.ends_with("/Auth/Keys/jellyfin-key")),
        "the media server's key minted for the dashboard was not revoked: {revoked:?}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A media server key that would not be revoked is kept in the file, so the next run
/// can still find which key it was.
#[tokio::test]
async fn a_key_that_would_not_be_revoked_is_not_forgotten() {
    let env = recorded_admin("unrevoked");
    let _ = store::set(
        &env,
        crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        "minted-earlier",
    );
    let _ = store::set(&env, "JELLYFIN_API_KEY", "jellyfin-key");
    let http = Fake::by_path(vec![
        (
            "/Users/AuthenticateByName",
            Answer::reply(200, r#"{"AccessToken":"t"}"#),
        ),
        ("/Auth/Keys", Answer::reply(500, "")),
    ]);
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
        .with_http(http)
        .with_filesystem(Arc::new(SeedFs::keyed(None, None)));

    let _ = super::super::published::publish_keys(
        &ctx,
        &[jellyfin_svc()],
        Some(std::path::Path::new("/opt/lemonfiber/stack")),
        None,
    )
    .await;

    let written = std::fs::read_to_string(&env).unwrap_or_default();
    assert!(
        written.contains("JELLYFIN_API_KEY=jellyfin-key"),
        "a key still valid on the server was forgotten: {written}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A rehearsal of a stack whose services have written no key names nothing, in the
/// same words a real run uses for it.
///
/// The names are the whole of what this connection would do, and where there are
/// none the honest answer is the one a real pass gives: the services write their
/// keys on first start and this stack has not got that far. Reporting a connection
/// that would be made would have an operator waiting for settings no later run is
/// going to fill in either.
#[tokio::test]
async fn a_rehearsed_publish_of_a_stack_with_no_keys_yet_names_nothing() {
    let env = config_scratch("publish-rehearsed");
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.to_path_buf()))
        .with_filesystem(Arc::new(SeedFs::keyed(None, None)))
        .rehearsing();

    let wiring = super::super::published::publish_keys(
        &ctx,
        &[arr("sonarr", 8989, "tv")],
        Some(std::path::Path::new("/opt/lemonfiber/stack")),
        None,
    )
    .await;

    assert!(is_skipped(&wiring), "{wiring:?}");
    let said = format!("{wiring:?}");
    assert!(
        said.contains("a later run completes it"),
        "a stack still starting was reported as one with nothing coming: {said}"
    );
    assert!(
        !env.exists(),
        "a run that had nothing to publish wrote a settings file anyway"
    );
}

/// A rehearsal names the settings it would fill and none of their values, and touches
/// no service on the way.
///
/// Claiming the listening server and revoking the media server's key are both writes,
/// so a rehearsal asks neither server anything; what it reports is what the services
/// already wrote down.
#[tokio::test]
async fn a_rehearsed_publish_names_the_settings_and_none_of_the_keys() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let env = config_scratch("publish-rehearsed-keys");
    let http = Fake::always(Answer::reply(200, r#"{"isInit":false}"#));
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.to_path_buf()))
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)))
        .rehearsing();

    let wiring = super::super::published::publish_keys(
        &ctx,
        &[arr("sonarr", 8989, "tv"), audiobookshelf_svc()],
        Some(std::path::Path::new("/opt/lemonfiber/stack")),
        None,
    )
    .await;

    assert_eq!(
        wiring.state,
        crate::seed::State::WouldWire {
            yours: None,
            ours: Some("SONARR_API_KEY".to_owned()),
        },
        "{wiring:?}"
    );
    assert!(
        !format!("{wiring:?}").contains("the-key"),
        "a rehearsal carried a key: {wiring:?}"
    );
    assert!(
        http.requests().is_empty(),
        "a rehearsal asked a server something: {:?}",
        http.requests()
    );
    assert!(!env.exists(), "a rehearsal wrote a settings file");
}

/// A service whose manifest entry names no file publishes no key.
///
/// The request service was declared that way until its key was found to be in a
/// file — an entry saying the key comes from somewhere this cannot read leaves
/// nothing to publish, and an older stack pinned here still says so. Publishing an
/// empty value instead would have whatever reads it authenticate with it and be
/// refused, which reads as a broken service rather than an unconfigured one.
#[tokio::test]
async fn a_service_whose_entry_names_no_file_publishes_no_key() {
    let env = recorded_admin("no-file-named");
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone())).with_filesystem(Arc::new(
        SeedFs::keyed(None, None).with_seerr(r#"{"main":{"apiKey":"unreachable"}}"#),
    ));
    let mut pathless = seerr_with_settings();
    pathless.api = Some(lemonfiber_manifest::Api {
        kind: lemonfiber_manifest::ApiKind::Seerr,
        key_source: lemonfiber_manifest::KeySource::ApiSettings,
        path: None,
        version: None,
    });

    let wiring = super::super::published::publish_keys(
        &ctx,
        &[pathless],
        Some(std::path::Path::new("/opt/lemonfiber/stack")),
        None,
    )
    .await;

    assert!(is_skipped(&wiring), "{wiring:?}");
    let written = std::fs::read_to_string(&env).unwrap_or_default();
    assert!(
        !written.contains("SEERR_API_KEY"),
        "a key was published for an entry naming no file: {written}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A listening server with no account gets one, and its password is kept.
///
/// Its root account goes to whoever makes the first one, from anywhere on the
/// network, so it is claimed here. Nothing is signed in for: no token is published,
/// since nothing in the stack reads one.
#[tokio::test]
async fn a_listening_server_with_no_account_is_given_one() {
    let env = recorded_admin("listening-fresh");
    let http = Fake::by_path(vec![
        ("/status", Answer::reply(200, r#"{"isInit":false}"#)),
        ("/init", Answer::reply(200, "")),
    ]);
    let ctx = seed_ctx(None, true, Vec::new(), Some(vec![9; 32]), Some(env.clone()))
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(None, None)));

    let wiring = super::super::published::publish_keys(
        &ctx,
        &[audiobookshelf_svc()],
        Some(std::path::Path::new("/opt/lemonfiber/stack")),
        None,
    )
    .await;

    assert!(is_skipped(&wiring), "{wiring:?}");
    let written = std::fs::read_to_string(&env).unwrap_or_default();
    assert!(
        !written.contains("AUDIOBOOKSHELF_API_KEY"),
        "a token was published for a dashboard that reads none: {written}"
    );
    assert!(
        written.contains("AUDIOBOOKSHELF_PASSWORD="),
        "the password it was made with was not kept: {written}"
    );
    assert!(
        http.requests()
            .iter()
            .any(|asked| asked.url.contains("/init")),
        "no account was made"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A listening server that already has an account is left as it is.
///
/// Somebody made that account, and its password is theirs: making a second is refused
/// by the server, and recording a password for an account this did not make would be
/// recording one that signs in to nothing.
#[tokio::test]
async fn a_listening_server_somebody_already_claimed_is_left_alone() {
    let env = recorded_admin("set-up-elsewhere");
    let http = Fake::by_path(vec![("/status", Answer::reply(200, r#"{"isInit":true}"#))]);
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(None, None)));

    let wiring = super::super::published::publish_keys(
        &ctx,
        &[audiobookshelf_svc()],
        Some(std::path::Path::new("/opt/lemonfiber/stack")),
        None,
    )
    .await;

    assert!(is_skipped(&wiring), "{wiring:?}");
    let written = std::fs::read_to_string(&env).unwrap_or_default();
    assert!(
        !written.contains("AUDIOBOOKSHELF_PASSWORD"),
        "a password was recorded for an account somebody else made: {written}"
    );
    assert!(
        !http
            .requests()
            .iter()
            .any(|asked| asked.url.contains("/init")),
        "a second account was asked for"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A stack whose services have written no key yet is skipped, not failed.
///
/// An empty value is worse than an absent one here: the quality sync refuses its
/// whole configuration over a single defined-but-empty name, so publishing
/// nothing leaves it working on the run after the keys exist.
#[tokio::test]
async fn nothing_is_published_where_no_service_has_written_a_key() {
    let env = recorded_admin("unpublished");
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
        .with_filesystem(Arc::new(SeedFs::keyed(None, None)));

    let wiring = super::super::published::publish_keys(
        &ctx,
        &[arr("sonarr", 8989, "tv")],
        Some(std::path::Path::new("/opt/lemonfiber/stack")),
        None,
    )
    .await;

    assert!(
        matches!(wiring.state, crate::seed::State::Skipped { .. }),
        "{wiring:?}"
    );
    let written = std::fs::read_to_string(&env).unwrap_or_default();
    assert!(
        !written.contains("SONARR_API_KEY"),
        "a name was published with nothing behind it: {written}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}
