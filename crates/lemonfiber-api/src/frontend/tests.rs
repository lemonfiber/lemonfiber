use lemonfiber_core::screening::Origin;

use super::{playing_from, POLICY};

/// A page playing from the front door may fetch and play media from it and nowhere new
/// besides, and keeps every other limit the page has.
#[test]
fn a_page_playing_from_the_front_door_loads_media_and_fetches_from_it_alone() {
    let policy = Origin::of("https://house.local:8920/watch?x=1")
        .as_ref()
        .map(playing_from)
        .unwrap_or_default();
    let directive = |name: &str| {
        policy
            .split(';')
            .map(str::trim)
            .find(|one| one.starts_with(name))
            .map(str::to_owned)
    };
    assert_eq!(
        directive("media-src").as_deref(),
        Some("media-src 'self' blob: https://house.local:8920")
    );
    assert_eq!(
        directive("connect-src").as_deref(),
        Some("connect-src 'self' https://house.local:8920")
    );
    for kept in [
        "default-src",
        "img-src",
        "style-src",
        "font-src",
        "base-uri",
        "frame-ancestors",
    ] {
        let page = POLICY
            .split(';')
            .map(str::trim)
            .find(|one| one.starts_with(kept));
        assert_eq!(directive(kept).as_deref(), page, "{kept}");
    }
}

/// Nothing that would widen the policy reaches it: an address whose host carries a
/// space, a `;`, a `*` or a quoted source is no origin a page plays from.
#[test]
fn a_hostile_front_door_never_reaches_the_policy() {
    for hostile in [
        "https://house.local; script-src *",
        "https://house.local;script-src",
        "https://*.house.local",
        "https://'unsafe-inline'",
        "https://house.local 'unsafe-inline'",
        "javascript://house.local",
        "data:text/html,hi",
        "house.local",
    ] {
        assert_eq!(Origin::of(hostile), None, "{hostile}");
    }
    let policy = Origin::of("http://192.168.1.4:8096")
        .as_ref()
        .map(playing_from)
        .unwrap_or_default();
    assert!(
        policy.contains("media-src 'self' blob: http://192.168.1.4:8096;"),
        "{policy}"
    );
    assert_eq!(policy.matches('*').count(), 0, "{policy}");
}
