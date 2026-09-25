//! Handing the *arrs to the subtitle finder.

use super::*;

/// The settings the finder reports before anything has told it anything.
const WATCHING_NOTHING: &str = r#"{"general":{"use_sonarr":false,"use_radarr":false}}"#;

/// The stack a subtitle test runs against: both \*arrs and the finder.
fn subtitle_stack() -> Vec<lemonfiber_manifest::Service> {
    vec![
        arr("sonarr", 8989, "tv"),
        arr("radarr", 7878, "movies"),
        bazarr_svc(),
    ]
}

/// A context whose filesystem answers both the \*arrs' keys and the finder's.
fn subtitle_ctx(http: Arc<Fake>, finder: Option<&'static str>) -> Ctx {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let mut fs = SeedFs::keyed(Some(KEYED), None);
    if let Some(config) = finder {
        fs = fs.with_bazarr(config);
    }
    seed_ctx(None, true, Vec::new(), None, None)
        .with_http(http)
        .with_filesystem(Arc::new(fs))
}

/// Both \*arrs are handed to the subtitle finder, each under its own section.
///
/// Asserted on the bodies that went out rather than only on the reported state:
/// the finder takes a form whose field names are its configuration's own paths
/// flattened, so a wiring that reported success while naming a field wrongly
/// would be a setting silently not set — which is exactly what this connection
/// failing looks like from the outside.
#[tokio::test]
async fn both_arrs_are_handed_to_the_subtitle_finder() {
    let http = Fake::by_path(vec![(
        "/api/system/settings",
        Answer::reply(200, WATCHING_NOTHING),
    )]);
    let ctx = subtitle_ctx(http.clone(), Some(FINDER_CONFIG));

    let wirings =
        super::super::subtitles::seed_subtitles(&ctx, &subtitle_stack(), Some(stack_root())).await;

    assert_eq!(wirings.len(), 2, "{wirings:?}");
    assert!(
        wirings
            .iter()
            .all(|wiring| wiring.state == crate::seed::State::Wired),
        "the finder was not told about both *arrs: {wirings:?}"
    );
    let bodies: Vec<String> = http
        .requests()
        .into_iter()
        .filter_map(|asked| asked.body)
        .collect();
    let written = bodies.join(" ");
    for expected in [
        "settings-general-use_sonarr=true",
        "settings-sonarr-ip=sonarr",
        "settings-sonarr-port=8989",
        "settings-general-use_radarr=true",
        "settings-radarr-ip=radarr",
        "settings-radarr-port=7878",
    ] {
        assert!(
            written.contains(expected),
            "{expected} missing from {written}"
        );
    }
}

/// The key the finder is reached with is its own, not the one filed beside it.
///
/// Its configuration holds an `apikey` under `auth` and another under each \*arr,
/// so a reader that took the first would present another service's credential —
/// and the finder would refuse it.
#[tokio::test]
async fn the_finder_is_reached_with_its_own_key() {
    let http = Fake::by_path(vec![(
        "/api/system/settings",
        Answer::reply(200, WATCHING_NOTHING),
    )]);
    let ctx = subtitle_ctx(http.clone(), Some(FINDER_CONFIG));

    let _ =
        super::super::subtitles::seed_subtitles(&ctx, &subtitle_stack(), Some(stack_root())).await;

    let presented: Vec<String> = http
        .requests()
        .into_iter()
        .flat_map(|asked| asked.headers)
        .filter(|(name, _)| name == "X-API-KEY")
        .map(|(_, value)| value)
        .collect();
    assert!(
        !presented.is_empty() && presented.iter().all(|key| key == "finder-key"),
        "the finder was reached with something other than its own key: {presented:?}"
    );
}

/// A finder already pointed at an \*arr is left alone rather than written again.
#[tokio::test]
async fn an_arr_the_finder_already_watches_is_left_as_it_is() {
    const HOLDING_BOTH: &str = r#"{
        "general": { "use_sonarr": true, "use_radarr": true },
        "sonarr": { "ip": "sonarr", "port": 8989, "apikey": "set" },
        "radarr": { "ip": "radarr", "port": 7878, "apikey": "set" }
    }"#;
    let http = Fake::by_path(vec![(
        "/api/system/settings",
        Answer::reply(200, HOLDING_BOTH),
    )]);
    let ctx = subtitle_ctx(http.clone(), Some(FINDER_CONFIG));

    let wirings =
        super::super::subtitles::seed_subtitles(&ctx, &subtitle_stack(), Some(stack_root())).await;

    // An `all` over an empty list is true, so the count is asserted first:
    // a step that wired nothing would otherwise satisfy every check below.
    assert_eq!(wirings.len(), 2, "{wirings:?}");
    assert!(
        wirings
            .iter()
            .all(|wiring| wiring.state == crate::seed::State::AlreadyWired),
        "{wirings:?}"
    );
    assert!(
        http.requests().iter().all(|asked| asked.body.is_none()),
        "a finder that already watched both was written to anyway"
    );
}

/// A stack with no subtitle finder wires nothing, rather than reporting a fault.
#[tokio::test]
async fn a_stack_without_a_subtitle_finder_has_nothing_to_wire() {
    let http = Fake::by_path(vec![(
        "/api/system/settings",
        Answer::reply(200, WATCHING_NOTHING),
    )]);
    let ctx = subtitle_ctx(http, Some(FINDER_CONFIG));
    let without = vec![arr("sonarr", 8989, "tv")];

    assert!(
        super::super::subtitles::seed_subtitles(&ctx, &without, Some(stack_root()))
            .await
            .is_empty()
    );
}

/// A finder that has not written its key yet is left for a later run.
///
/// It is a service still starting rather than a fault, and the run that finds
/// it started completes the wiring — so nothing is reported against it here.
#[tokio::test]
async fn a_finder_that_has_written_no_key_yet_is_left_for_a_later_run() {
    let http = Fake::by_path(vec![(
        "/api/system/settings",
        Answer::reply(200, WATCHING_NOTHING),
    )]);
    let ctx = subtitle_ctx(http, None);

    assert!(
        super::super::subtitles::seed_subtitles(&ctx, &subtitle_stack(), Some(stack_root()))
            .await
            .is_empty()
    );
}

/// An \*arr that has not written its key yet is skipped, and said to be skipped.
///
/// The finder needs the \*arr's own key to read anything from it, so wiring it
/// without one would point the finder at a service it cannot read.
#[tokio::test]
async fn an_arr_with_no_key_yet_is_skipped_rather_than_wired() {
    let http = Fake::by_path(vec![(
        "/api/system/settings",
        Answer::reply(200, WATCHING_NOTHING),
    )]);
    let ctx = seed_ctx(None, true, Vec::new(), None, None)
        .with_http(http)
        .with_filesystem(Arc::new(
            SeedFs::keyed(None, None).with_bazarr(FINDER_CONFIG),
        ));

    let wirings =
        super::super::subtitles::seed_subtitles(&ctx, &subtitle_stack(), Some(stack_root())).await;

    // An `all` over an empty list is true, so the count is asserted first:
    // a step that wired nothing would otherwise satisfy every check below.
    assert_eq!(wirings.len(), 2, "{wirings:?}");
    assert!(
        wirings
            .iter()
            .all(|wiring| matches!(wiring.state, crate::seed::State::Skipped { .. })),
        "{wirings:?}"
    );
}

/// An \\*arr filing media that carries no subtitles is passed over.
///
/// The finder has no section for music, so wiring one would mean writing
/// settings under a name it does not read. Driven with Lidarr in the stack
/// because the stack has one.
#[tokio::test]
async fn an_arr_filing_media_with_no_subtitles_is_passed_over() {
    let http = Fake::by_path(vec![(
        "/api/system/settings",
        Answer::reply(200, WATCHING_NOTHING),
    )]);
    let ctx = subtitle_ctx(http, Some(FINDER_CONFIG));
    let with_music = vec![
        arr("sonarr", 8989, "tv"),
        arr("lidarr", 8686, "music"),
        bazarr_svc(),
    ];

    let wirings =
        super::super::subtitles::seed_subtitles(&ctx, &with_music, Some(stack_root())).await;

    assert_eq!(
        wirings.len(),
        1,
        "the music *arr was wired too: {wirings:?}"
    );
    assert!(
        wirings
            .first()
            .is_some_and(|wiring| wiring.connection.contains("sonarr")),
        "{wirings:?}"
    );
}

/// An \\*arr the stack publishes no port for is passed over.
///
/// The finder is told a name and a port together; without one there is no
/// address to hand it, and half an address is worse than none — it would be
/// written, look wired, and never answer.
#[tokio::test]
async fn an_arr_with_no_port_declared_is_passed_over() {
    let http = Fake::by_path(vec![(
        "/api/system/settings",
        Answer::reply(200, WATCHING_NOTHING),
    )]);
    let ctx = subtitle_ctx(http, Some(FINDER_CONFIG));
    let mut portless = arr("sonarr", 8989, "tv");
    portless.port = None;

    let wirings = super::super::subtitles::seed_subtitles(
        &ctx,
        &[portless, bazarr_svc()],
        Some(stack_root()),
    )
    .await;

    assert!(wirings.is_empty(), "{wirings:?}");
}

/// A finder whose manifest entry names no configuration path is no target.
///
/// Its key is only ever read from a file, so an entry that does not say where
/// that file is leaves nothing to read — passed over rather than guessed at.
#[tokio::test]
async fn a_finder_with_no_configuration_path_is_no_target() {
    let http = Fake::by_path(vec![(
        "/api/system/settings",
        Answer::reply(200, WATCHING_NOTHING),
    )]);
    let ctx = subtitle_ctx(http, Some(FINDER_CONFIG));
    let mut pathless = bazarr_svc();
    pathless.api = Some(lemonfiber_manifest::Api {
        kind: lemonfiber_manifest::ApiKind::Bazarr,
        key_source: lemonfiber_manifest::KeySource::ConfigYaml,
        path: None,
        version: None,
    });

    let wirings = super::super::subtitles::seed_subtitles(
        &ctx,
        &[arr("sonarr", 8989, "tv"), pathless],
        Some(stack_root()),
    )
    .await;

    assert!(wirings.is_empty(), "{wirings:?}");
}

/// A finder that will not answer is reported as failed, in its own words.
#[tokio::test]
async fn a_finder_that_refuses_is_reported_rather_than_passed_over() {
    let http = Fake::by_path(vec![("/api/system/settings", Answer::reply(500, ""))]);
    let ctx = subtitle_ctx(http, Some(FINDER_CONFIG));

    let wirings =
        super::super::subtitles::seed_subtitles(&ctx, &subtitle_stack(), Some(stack_root())).await;

    // An `all` over an empty list is true, so the count is asserted first:
    // a step that wired nothing would otherwise satisfy every check below.
    assert_eq!(wirings.len(), 2, "{wirings:?}");
    assert!(
        wirings
            .iter()
            .all(|wiring| matches!(wiring.state, crate::seed::State::Failed { .. })),
        "{wirings:?}"
    );
}

/// A finder that takes the read and refuses the write is reported as failed.
///
/// The two halves fail separately: reading what it holds is what decides whether
/// to write at all, so a write refused after a read that succeeded is a distinct
/// path from a finder that never answered.
#[tokio::test]
async fn a_write_the_finder_refuses_is_reported() {
    let http = Fake::by_path_in_turn(vec![(
        "/api/system/settings",
        vec![
            Answer::reply(200, WATCHING_NOTHING),
            Answer::reply(401, ""),
            Answer::reply(200, WATCHING_NOTHING),
            Answer::reply(401, ""),
        ],
    )]);
    let ctx = subtitle_ctx(http, Some(FINDER_CONFIG));

    let wirings =
        super::super::subtitles::seed_subtitles(&ctx, &subtitle_stack(), Some(stack_root())).await;

    // An `all` over an empty list is true, so the count is asserted first:
    // a step that wired nothing would otherwise satisfy every check below.
    assert_eq!(wirings.len(), 2, "{wirings:?}");
    assert!(
        wirings
            .iter()
            .all(|wiring| matches!(wiring.state, crate::seed::State::Failed { .. })),
        "{wirings:?}"
    );
}

/// What the finder holds: pointed at one \*arr somewhere else, and not watching the
/// other at all.
const WATCHING_ELSEWHERE: &str = r#"{
    "general": { "use_sonarr": true, "use_radarr": false },
    "sonarr": { "ip": "somewhere-else", "port": 1234, "apikey": "set" }
}"#;

/// A rehearsal says where the finder is looking now and where it would be pointed,
/// and points it nowhere.
///
/// Two \*arrs, because what the finder holds is not one shape. One it is already
/// watching at the wrong address with a key, and the other it is not watching at
/// all — and the difference matters to the operator reading this: the first is a
/// setting of theirs about to be replaced, the second is a connection that has
/// never existed. The key is named too, because a finder pointed at the right
/// \*arr with no key is exactly the case this connection exists to fix, and two
/// addresses on their own would read as a change to nothing.
#[tokio::test]
async fn a_rehearsed_pass_says_where_the_finder_looks_now_and_points_it_nowhere() {
    let http = Fake::by_path(vec![(
        "/api/system/settings",
        Answer::reply(200, WATCHING_ELSEWHERE),
    )]);
    let ctx = subtitle_ctx(http.clone(), Some(FINDER_CONFIG)).rehearsing();

    let wirings =
        super::super::subtitles::seed_subtitles(&ctx, &subtitle_stack(), Some(stack_root())).await;

    let states: Vec<crate::seed::State> =
        wirings.iter().map(|wiring| wiring.state.clone()).collect();
    assert_eq!(
        states,
        vec![
            crate::seed::State::WouldWire {
                yours: Some("somewhere-else:1234".to_owned()),
                ours: Some("sonarr:8989, with lemonfiber's key".to_owned()),
            },
            crate::seed::State::WouldWire {
                yours: None,
                ours: Some("radarr:7878, with lemonfiber's key".to_owned()),
            },
        ],
        "{wirings:?}"
    );
    assert!(
        http.requests().iter().all(|asked| asked.body.is_none()),
        "a rehearsal pointed the finder at something"
    );
}
