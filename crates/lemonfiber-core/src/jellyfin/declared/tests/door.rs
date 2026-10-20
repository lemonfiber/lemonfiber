//! The door's own cases, `check_door.py`'s in the media stack, each asked of a guard
//! enforcing Jellyfin's declaration and nothing else. The four that turn on which client
//! a request came from are the envelope's proxy, not the declaration, and are not here.

use super::declared;
use super::guard::{matching, passes, CLOSED, MEMBER, OPEN};

/// One case: the request it makes, what it presents, and whether the door passes it.
#[derive(serde::Deserialize)]
struct Case {
    name: String,
    path: String,
    presents: String,
    passes: bool,
}

/// `id` written with the dashes Jellyfin also reads.
fn dashed(id: &str) -> String {
    [(0, 8), (8, 12), (12, 16), (16, 20), (20, 32)]
        .iter()
        .filter_map(|&(from, to)| id.get(from..to))
        .collect::<Vec<_>>()
        .join("-")
}

/// The request target a case makes, its placeholders filled.
fn target(path: &str) -> String {
    path.replace("{open_dashed}", &dashed(OPEN))
        .replace("{closed_dashed}", &dashed(CLOSED))
        .replace("{closed_upper}", &CLOSED.to_ascii_uppercase())
        .replace("{token_upper}", &MEMBER.to_ascii_uppercase())
        .replace("{open}", OPEN)
        .replace("{closed}", CLOSED)
        .replace("{token}", MEMBER)
}

/// The headers a case presents its member's session in.
fn headers(presents: &str) -> Vec<(String, String)> {
    let unknown = "0".repeat(32);
    let header = match presents {
        "authorization" => Some((
            "Authorization",
            format!(
                r#"MediaBrowser Client="door", Device="door", DeviceId="door", Version="1", Token="{MEMBER}""#
            ),
        )),
        "x-emby-token" => Some(("X-Emby-Token", MEMBER.to_owned())),
        "x-mediabrowser-token" => Some(("X-MediaBrowser-Token", MEMBER.to_owned())),
        "unknown" => Some(("X-Emby-Token", unknown)),
        "bearer" => Some(("Authorization", format!("Bearer {MEMBER}"))),
        "bearer-unknown" => Some(("Authorization", format!("Bearer {unknown}"))),
        "bearer-smuggled" => Some((
            "Authorization",
            format!(r#"Bearer {MEMBER}", DeviceId="door"#),
        )),
        _ => None,
    };
    header
        .map(|(name, value)| vec![(name.to_owned(), value)])
        .unwrap_or_default()
}

#[test]
fn a_guard_enforcing_the_declaration_answers_every_case_of_the_door_as_the_door_does() {
    let cases: Vec<Case> = serde_json::from_str(include_str!("door.json")).unwrap_or_default();
    assert_eq!(cases.len(), 87);
    let declared = declared();
    let differing: Vec<&str> = cases
        .iter()
        .filter(|case| {
            passes(&declared, &target(&case.path), &headers(&case.presents)) != case.passes
        })
        .map(|case| case.name.as_str())
        .collect();
    assert_eq!(differing, Vec::<&str>::new());
}

#[test]
fn every_pattern_and_shape_the_declaration_holds_compiles() {
    let declared = declared();
    let mut patterns: Vec<&str> = declared.refused.iter().map(String::as_str).collect();
    for path in &declared.bytes {
        patterns.push(&path.pattern);
        patterns.push(&path.item);
        patterns.extend(path.exempt.iter().map(String::as_str));
    }
    patterns.push(&declared.token.shape);
    for pattern in patterns {
        assert!(regex::Regex::new(pattern).is_ok(), "{pattern}");
    }
    assert!(matching(&declared.token.shape, MEMBER));
}

#[test]
fn a_byte_route_naming_an_id_with_more_before_or_after_it_is_refused() {
    let declared = declared();
    let session = headers("authorization");
    assert!(passes(
        &declared,
        &target("/Videos/{open}/stream"),
        &session
    ));
    for path in [
        "/Videos/x{open}/stream",
        "/Videos/{open}x/stream",
        "/Items/00{open}/Images/Primary",
        "/Videos/{open}/Trickplay/320/tiles.m3u8?MediaSourceId=x{open}",
    ] {
        assert!(!passes(&declared, &target(path), &session), "{path}");
    }
    for path in &declared.bytes {
        assert!(!matching(&path.item, &format!("x{OPEN}")));
        assert!(!matching(&path.item, &format!("{OPEN}x")));
    }
}
