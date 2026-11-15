use super::{playing_from, POLICY};

/// A page playing from the front door may fetch and play media from it and nowhere new
/// besides, and keeps every other limit the page has.
#[test]
fn a_page_playing_from_the_front_door_loads_media_and_fetches_from_it_alone() {
    let door = "https://house.local:8920";
    let policy = playing_from(door);
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
