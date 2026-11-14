#![no_main]
//! The pass service reads a forwarded request and the pass inside it from whatever the
//! guard was sent, and answers yes to nothing it cannot hold to one shape: every split,
//! decode and open on that path is fed here.

use lemonfiber_core::pass::{InQuery, ItemPath, Key, LivePath, Paths, Renewals, Watching, KEY_BYTES};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let key = Key::of([1; KEY_BYTES]);
    let _ = key.confirms(text, "stream", 0, &Renewals::none());
    let _ = key.open(text, "item", 0, &Renewals::none());
    let Ok(watching) = Watching::of(&Paths::of(
        vec![
            LivePath {
                pattern: "(?i)^/live/(?P<stream>[^/]+)/".to_owned(),
                parameter: None,
                stream: "^[0-9a-f]{32}$".to_owned(),
            },
            LivePath {
                pattern: "(?i)^/videos/[^/]+/live\\.m3u8$".to_owned(),
                parameter: Some("LiveStreamId".to_owned()),
                stream: "^[0-9a-f_]{98}$".to_owned(),
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
                    pattern: "/library/(?P<item>[0-9]+)".to_owned(),
                }),
                item: "^[0-9]+$".to_owned(),
            },
        ],
    )) else {
        return;
    };
    let _ = watching.confirms(&key, text, 0, &Renewals::none());
});
