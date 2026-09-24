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
/// Three of them are configured by the environment and by nothing else — there is
/// no endpoint to tell them anything — so a key that is read and not written down
/// is one the quality sync, the archive extractor and the dashboard all run
/// without. Asserted on the file, because that is the whole of the mechanism.
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

/// A service the dashboard has a widget for is published even where no \*arr list
/// holds it.
///
/// The keys were published by walking the media-filing \*arrs, which is the right
/// set for root folders and download clients and the wrong one here: Prowlarr
/// manages no media and so declares no media types, and the subtitle finder is not
/// Servarr-shaped at all. Both have a key, both have a widget, and neither key was
/// written — so those widgets asked with an empty credential and were refused.
/// qBittorrent is reached by name and password rather than by key, and half a
/// credential authenticates as badly as none.
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
    // The media server signs in and lists its keys; the listening server says it
    // already has an account and hands a token back on sign-in. Both need a
    // password already recorded, which is what a run that made them would leave.
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
        (
            "/login",
            Answer::reply(200, r#"{"user":{"token":"listening-token"}}"#),
        ),
    ]);
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
        .with_http(http)
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
        "JELLYFIN_API_KEY=jellyfin-key",
        "AUDIOBOOKSHELF_API_KEY=listening-token",
        "QBITTORRENT_USERNAME=admin",
    ] {
        assert!(
            written.contains(expected),
            "{expected} was not published: {written}"
        );
    }
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
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
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

/// A service whose manifest entry names no file publishes no key.
///
/// The request service was declared that way until its key was found to be in a
/// file — an entry saying the key comes from somewhere this cannot read leaves
/// nothing to publish, and an older stack pinned here still says so. Publishing an
/// empty value instead would have the dashboard authenticate with it and be
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
/// The token is not: the service hands back the same one on every sign-in, so what
/// is recorded is the password it was made with and the token is asked for again.
#[tokio::test]
async fn a_listening_server_with_no_account_is_given_one() {
    let env = recorded_admin("listening-fresh");
    let http = Fake::by_path(vec![
        ("/status", Answer::reply(200, r#"{"isInit":false}"#)),
        ("/init", Answer::reply(200, "")),
        (
            "/login",
            Answer::reply(200, r#"{"user":{"token":"listening-token"}}"#),
        ),
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

    assert_eq!(wiring.state, crate::seed::State::Wired, "{wiring:?}");
    let written = std::fs::read_to_string(&env).unwrap_or_default();
    assert!(
        written.contains("AUDIOBOOKSHELF_API_KEY=listening-token"),
        "the token was not published: {written}"
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

/// A server somebody else set up publishes nothing, rather than a wrong key.
///
/// Both of these are reached with a password lemonfiber minted. Where one already
/// has an account and no password is recorded, there is no way to sign in — and a
/// key published from a failed sign-in would be an empty one, which the dashboard
/// would authenticate with and be refused.
#[tokio::test]
async fn a_server_set_up_by_somebody_else_publishes_nothing() {
    let env = recorded_admin("set-up-elsewhere");
    let http = Fake::by_path(vec![
        ("/status", Answer::reply(200, r#"{"isInit":true}"#)),
        ("/Users/AuthenticateByName", Answer::reply(401, "")),
    ]);
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
        .with_http(http)
        .with_filesystem(Arc::new(SeedFs::keyed(None, None)));

    let wiring = super::super::published::publish_keys(
        &ctx,
        &[audiobookshelf_svc(), jellyfin_svc()],
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
        !written.contains("AUDIOBOOKSHELF_API_KEY") && !written.contains("JELLYFIN_API_KEY"),
        "a key was published for a server that could not be signed in to: {written}"
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
