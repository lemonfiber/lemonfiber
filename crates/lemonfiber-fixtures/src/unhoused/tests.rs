use super::unhoused;

/// What a member may not be shown is found, whichever kind of thing it is.
#[test]
fn what_a_member_may_not_be_shown_is_found() {
    for (text, found) in [
        ("Jellyfin did not answer", "jellyfin"),
        ("The media server did not answer", "media server"),
        ("It needs 4 GB more", "4"),
        ("It needs 12GB more", "12gb"),
        ("Fetching at 3 MB/s", "3"),
        ("Open http://house.lan", "http"),
        ("Reached at 192.168.1.20", "192.168.1.20"),
        ("Now on v1.4.2", "v1.4.2"),
        ("The stack is stopped", "stack"),
        ("It is still downloading", "downloading"),
        ("No indexer answered", "indexer"),
    ] {
        assert_eq!(unhoused(text).as_deref(), Some(found), "{text}");
    }
}

/// The household's own words pass, the house included, and a word is matched whole.
#[test]
fn the_households_own_words_pass() {
    for text in [
        "This isn't working right now, and whoever runs the house has been told.",
        "It is already in the house. Play it from your shelf.",
        "Ask again once your allowance frees up.",
        "Your library isn't answering right now.",
        "The idea is sound, and serving dinner can wait.",
        "Episode 3 of season 2.",
    ] {
        assert_eq!(unhoused(text), None, "{text}");
    }
}

/// What the contract publishes names every part the bundle pins, and each ruled example
/// names what it is ruled for.
#[test]
fn what_is_published_names_every_pinned_part_and_what_each_example_breaks() {
    let published = super::published();
    let listed = |key: &str| -> Vec<String> {
        published
            .get(key)
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|one| one.as_str().map(str::to_owned))
            .collect()
    };
    let names = listed("names");
    for part in crate::parts::pinned() {
        assert!(names.contains(&part.replace('-', " ")), "{part}");
    }
    assert!(names.contains(&"plex".to_owned()));
    assert!(listed("terms").contains(&"indexer".to_owned()));
    assert!(listed("sizes").contains(&"gb".to_owned()));
    assert!(listed("marks").contains(&"://".to_owned()));
    assert_eq!(
        published
            .get("declared")
            .and_then(serde_json::Value::as_object)
            .map(|declared| declared.keys().cloned().collect::<Vec<_>>()),
        Some(vec!["form".to_owned(), "stack".to_owned()])
    );
    let examples = published
        .get("examples")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert_eq!(examples.len(), super::RULED.len());
    for example in examples {
        let text = example.get("text").and_then(serde_json::Value::as_str);
        let found = example.get("names").and_then(serde_json::Value::as_str);
        assert!(text.is_some() && found.is_some(), "{example}");
        assert_eq!(text.and_then(unhoused).as_deref(), found, "{example}");
    }
}
