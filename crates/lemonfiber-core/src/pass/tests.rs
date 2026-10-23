use lemonfiber_sidecar::pass::{Key, Live, LivePath, KEY_BYTES, LIFETIME};

use super::Watching;

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
    Watching::of(&Live::of(vec![LivePath {
        pattern: "^/live/(?P<stream>[^/]+)/".to_owned(),
        stream: "^[a-z0-9]{6}$".to_owned(),
    }]))
    .unwrap_or_else(|refused| unreachable!("the paths compile: {refused}"))
}

/// A request for one of `stream`'s segments carrying `pass`.
fn forwarded(stream: &str, pass: &str) -> String {
    format!("/live/{stream}/segment7.ts?lfpass={pass}")
}

#[test]
fn a_pass_good_for_the_live_stream_the_path_names_is_confirmed() {
    let pass = key().issue(STREAM, "member-1", NOW);

    assert!(watching().confirms(&key(), &forwarded(STREAM, &pass), NOW));
    assert!(!watching().confirms(&key(), &forwarded(STREAM, &pass), NOW + LIFETIME));
}

#[test]
fn a_pass_for_another_live_stream_or_a_path_naming_none_is_refused() {
    let other = key().issue("zzz999", "member-1", NOW);
    let pass = key().issue(STREAM, "member-1", NOW);

    assert!(!watching().confirms(&key(), &forwarded(STREAM, &other), NOW));
    assert!(!watching().confirms(&key(), &format!("/movies/{STREAM}/x?lfpass={pass}"), NOW));
    assert!(!watching().confirms(&key(), &forwarded("ABC123", &pass), NOW));
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
        assert!(
            !watching().confirms(&key(), &format!("{path}?lfpass={pass}"), NOW),
            "{path:?}"
        );
    }
    assert!(watching().confirms(&key(), &format!("/live/{STREAM}/s.ts?lfpass={pass}"), NOW));
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
        assert!(!watching().confirms(&key(), &carried, NOW), "{carried}");
    }
    assert!(watching().confirms(
        &key(),
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
        let live = Live::of(vec![LivePath {
            pattern: pattern.to_owned(),
            stream: stream.to_owned(),
        }]);
        assert!(Watching::of(&live).is_err(), "{pattern} {stream}");
    }
}
