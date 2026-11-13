use lemonfiber_sidecar::pass::{Key, LivePath, Paths, Renewals, KEY_BYTES, LIFETIME};

use super::{Confirmed, InQuery, ItemPath, Session, Watching, NONCE_BYTES};

/// The moment these passes are issued at.
const NOW: u64 = 1_800_000_000;

/// A live stream's id, in the shape the paths below declare.
const STREAM: &str = "abc123";

/// A key of no randomness, so a pass made with it is the same every run.
const fn key() -> Key {
    Key::of([3; KEY_BYTES])
}

/// The live-stream paths of a media server serving them under `/live/<id>/`.
fn watching() -> Watching {
    Watching::of(&Paths::of(
        vec![LivePath {
            pattern: "^/live/(?P<stream>[^/]+)/".to_owned(),
            parameter: None,
            stream: "^[a-z0-9]{6}$".to_owned(),
        }],
        Vec::new(),
    ))
    .unwrap_or_else(|refused| unreachable!("the paths compile: {refused}"))
}

/// Whether the request `forwarded` names is confirmed at `now` as carrying a pass good
/// for the live stream it names.
fn live(forwarded: &str, now: u64) -> bool {
    watching().confirms(&key(), forwarded, now, &Renewals::none()) == Some(Confirmed::Live)
}

/// A request for one of `stream`'s segments carrying `pass`.
fn forwarded(stream: &str, pass: &str) -> String {
    format!("/live/{stream}/segment7.ts?lfpass={pass}")
}

#[test]
fn a_pass_good_for_the_live_stream_the_path_names_is_confirmed() {
    let pass = key().issue(STREAM, "member-1", NOW);

    assert!(live(&forwarded(STREAM, &pass), NOW));
    assert!(!live(&forwarded(STREAM, &pass), NOW + LIFETIME));
}

#[test]
fn a_pass_for_another_live_stream_or_a_path_naming_none_is_refused() {
    let other = key().issue("zzz999", "member-1", NOW);
    let pass = key().issue(STREAM, "member-1", NOW);

    assert!(!live(&forwarded(STREAM, &other), NOW));
    assert!(!live(&format!("/movies/{STREAM}/x?lfpass={pass}"), NOW));
    assert!(!live(&forwarded("ABC123", &pass), NOW));
}

#[test]
fn a_path_the_upstream_could_read_otherwise_is_refused() {
    let pass = key().issue(STREAM, "member-1", NOW);
    for path in [
        format!("live/{STREAM}/s.ts"),
        format!("/live/{STREAM}/../zzz999/s.ts"),
        format!("/live/{STREAM}/./s.ts"),
        format!("/live/{STREAM}/s.ts/.."),
        format!("/live/{STREAM}//s.ts"),
        format!("/live/{STREAM}/%2e%2e/s.ts"),
        format!("/live/{STREAM}/..\\zzz999/s.ts"),
        format!("/live/{STREAM}/s.ts;x"),
        format!("/live/{STREAM}/s\u{7f}.ts"),
        format!("/live/{STREAM}/s\n.ts"),
        format!("/live/{STREAM}/s .ts"),
        format!("/live/{STREAM}/s\u{a0}.ts"),
    ] {
        assert!(!live(&format!("{path}?lfpass={pass}"), NOW), "{path:?}");
    }
    assert!(live(&format!("/live/{STREAM}/s.ts?lfpass={pass}"), NOW));
}

#[test]
fn a_pass_anywhere_but_one_lfpass_is_refused() {
    let pass = key().issue(STREAM, "member-1", NOW);
    for carried in [
        format!("/live/{STREAM}/s.ts"),
        format!("/live/{STREAM}/s.ts?lfpass"),
        format!("/live/{STREAM}/s.ts?LFPASS={pass}"),
        format!("/live/{STREAM}/s.ts?pass={pass}"),
        format!("/live/{STREAM}/s.ts?lfpass={pass}&lfpass={pass}"),
        format!("/live/{STREAM}/s.ts?lfpass={pass}#x"),
    ] {
        assert!(!live(&carried, NOW), "{carried}");
    }
    assert!(live(
        &format!("/live/{STREAM}/s.ts?other=1&lfpass={pass}"),
        NOW
    ));
}

#[test]
fn live_stream_paths_that_do_not_compile_or_place_no_stream_are_refused() {
    for (pattern, stream) in [
        ("^/live/(?P<stream>[^/]+", "^.+$"),
        ("^/live/(?P<item>[^/]+)/", "^.+$"),
        ("^/live/(?P<stream>[^/]+)/", "^(.+$"),
    ] {
        let live = Paths::of(
            vec![LivePath {
                pattern: pattern.to_owned(),
                parameter: None,
                stream: stream.to_owned(),
            }],
            Vec::new(),
        );
        assert!(Watching::of(&live).is_err(), "{pattern} {stream}");
    }
}

const DIGEST: &str = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945";

#[test]
fn a_pin_is_this_releases_own_image_at_its_digest() {
    assert_eq!(
        super::pin("0.18.0", DIGEST),
        format!("ghcr.io/lemonfiber/lemonfiber:0.18.0@{DIGEST}")
    );
}

#[test]
fn a_pin_is_held_only_for_the_release_it_names_and_only_by_a_digest() {
    let pin = super::pin("0.18.0", DIGEST);
    assert_eq!(
        super::pinned(&format!("{pin}\n"), "0.18.0"),
        Some(pin.clone())
    );
    assert_eq!(super::pinned(&pin, "0.18.1"), None);
    assert_eq!(super::pinned(&pin, "0.18"), None);
    assert_eq!(
        super::pinned(&super::pin("0.18.0", "sha256:abc"), "0.18.0"),
        None
    );
    assert_eq!(
        super::pinned(
            &format!("example.invalid/lemonfiber:0.18.0@{DIGEST}"),
            "0.18.0"
        ),
        None
    );
    assert_eq!(super::pinned(&format!("{pin} extra"), "0.18.0"), None);
}

#[test]
fn the_pass_service_is_started_answering_at_its_address() {
    assert_eq!(
        super::command(std::net::SocketAddr::from(([10, 80, 96, 34], 8090))),
        ["pass", "--listen", "10.80.96.34:8090"]
    );
}

/// A live stream's id as a transcoding route names it in its query.
const TRANSCODED: &str = "aaaa_bbbb_cccc";

/// An item's id, in the shape the byte paths below declare.
const ITEM: &str = "0123456789abcdef0123456789abcdef";

/// The paths of a media server naming a transcoded live stream in `LiveStreamId`, serving
/// an item under `/videos/<id>/` and a photo's item in the `url` of `/photo`.
fn everything() -> Watching {
    Watching::of(&Paths::of(
        vec![
            LivePath {
                pattern: "(?i)^/live/(?P<stream>[^/]+)/".to_owned(),
                parameter: None,
                stream: "^[a-z0-9]{6}$".to_owned(),
            },
            LivePath {
                pattern: "(?i)^/videos/[^/]+/(?:live|master)\\.m3u8$".to_owned(),
                parameter: Some("LiveStreamId".to_owned()),
                stream: "^[a-z]{4}_[a-z]{4}_[a-z]{4}$".to_owned(),
            },
        ],
        vec![
            ItemPath {
                pattern: "(?i)^/videos/(?P<item>[^/]+)/".to_owned(),
                query: None,
                item: "^[0-9a-f]{32}$".to_owned(),
            },
            ItemPath {
                pattern: "^/photo$".to_owned(),
                query: Some(InQuery {
                    parameter: "url".to_owned(),
                    pattern: "/library/(?P<item>[0-9a-f]+)".to_owned(),
                }),
                item: "^[0-9a-f]{32}$".to_owned(),
            },
        ],
    ))
    .unwrap_or_else(|refused| unreachable!("the paths compile: {refused}"))
}

#[test]
fn a_transcoded_live_stream_is_read_from_its_one_parameter_alone() {
    let pass = key().issue(TRANSCODED, "member-1", NOW);
    let confirmed =
        |forwarded: &str| everything().confirms(&key(), forwarded, NOW, &Renewals::none());
    let at = format!("/videos/{ITEM}/live.m3u8");

    assert_eq!(
        confirmed(&format!("{at}?LiveStreamId={TRANSCODED}&lfpass={pass}")),
        Some(Confirmed::Live)
    );
    assert_eq!(
        everything().stream(&at, &format!("LiveStreamId={TRANSCODED}")),
        Some(TRANSCODED.to_owned())
    );
    for refused in [
        format!("{at}?livestreamid={TRANSCODED}&lfpass={pass}"),
        format!("{at}?LiveStreamId={TRANSCODED}&LiveStreamId={TRANSCODED}&lfpass={pass}"),
        format!("{at}?LiveStreamId={TRANSCODED}&liveStreamId=x&lfpass={pass}"),
        format!("{at}?LiveStreamId=aaaa%5Fbbbb_cccc%25&lfpass={pass}"),
        format!("{at}?LiveStreamId=aaaa+bbbb_cccc&lfpass={pass}"),
        format!("{at}?LiveStreamId=zzzz_bbbb_cccc&lfpass={pass}"),
        format!("{at}?LiveStreamId=&lfpass={pass}"),
    ] {
        assert_eq!(confirmed(&refused), None, "{refused}");
    }
    assert_eq!(
        confirmed(&format!(
            "{at}?LiveStreamId=aaaa%5Fbbbb%5Fcccc&lfpass={pass}"
        )),
        Some(Confirmed::Live)
    );
}

/// The session the item passes below seal.
fn session() -> Session {
    Session {
        member: "member-1".to_owned(),
        token: "ab".repeat(16),
    }
}

/// An item pass for `item`, sealed at `NOW`.
fn sealed(item: &str) -> String {
    key()
        .seal(item, &session(), [9; NONCE_BYTES], NOW)
        .unwrap_or_else(|| unreachable!("a session seals"))
}

#[test]
fn an_item_pass_is_confirmed_with_its_session_on_its_items_routes_alone() {
    let pass = sealed(ITEM);
    let confirmed = |forwarded: &str, now: u64| {
        everything().confirms(&key(), forwarded, now, &Renewals::none())
    };

    for route in [
        format!("/videos/{ITEM}/master.m3u8?lfpass={pass}"),
        format!("/videos/{ITEM}/hls1/main/0.ts?runtimeTicks=0&lfpass={pass}"),
        format!("/photo?url=%2Flibrary%2F{ITEM}&lfpass={pass}"),
    ] {
        assert_eq!(
            confirmed(&route, NOW),
            Some(Confirmed::Item(session())),
            "{route}"
        );
        assert_eq!(confirmed(&route, NOW + LIFETIME), None, "{route}");
    }
    let other = "fedcba9876543210fedcba9876543210";
    for refused in [
        format!("/videos/{other}/master.m3u8?lfpass={pass}"),
        format!("/photo?url=%2Flibrary%2F{other}&lfpass={pass}"),
        format!("/photo?url=%2Flibrary%2F{ITEM}&url=%2Flibrary%2F{other}&lfpass={pass}"),
        format!(
            "/videos/{ITEM}/master.m3u8?lfpass={}",
            key().issue(ITEM, "member-1", NOW)
        ),
        format!("/elsewhere/{ITEM}?lfpass={pass}"),
    ] {
        assert_eq!(confirmed(&refused, NOW), None, "{refused}");
    }
}
