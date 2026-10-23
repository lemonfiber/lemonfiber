use super::{File, Key, Live, LivePath, FORMAT, KEY_BYTES, LIFETIME};

/// A key of no randomness at all, so a pass made with it is the same every run.
const fn key() -> Key {
    Key::of([7; KEY_BYTES])
}

/// A pass's three parts: the member, the moment it lapses and the MAC.
fn parts(pass: &str) -> (String, String, String) {
    let mut each = pass.split('.').map(str::to_owned);
    (
        each.next().unwrap_or_default(),
        each.next().unwrap_or_default(),
        each.next().unwrap_or_default(),
    )
}

/// The moment these passes are issued at.
const NOW: u64 = 1_800_000_000;

#[test]
fn a_pass_is_good_for_the_live_stream_it_was_issued_for_until_it_lapses() {
    let pass = key().issue("abc123", "member-1", NOW);

    assert!(key().confirms(&pass, "abc123", NOW));
    assert!(key().confirms(&pass, "abc123", NOW + LIFETIME - 1));
    assert!(!key().confirms(&pass, "abc123", NOW + LIFETIME));
    assert!(!key().confirms(&pass, "abc124", NOW));
}

#[test]
fn a_pass_made_with_another_key_or_altered_in_any_part_is_refused() {
    let pass = key().issue("abc123", "member-1", NOW);
    let (member, lapses, tag) = parts(&pass);
    let later = format!("{member}.{}.{tag}", NOW + 10 * LIFETIME);
    let (other_member, _, _) = parts(&key().issue("abc123", "member-2", NOW));
    let swapped = format!("{other_member}.{lapses}.{tag}");

    assert!(!Key::of([8; KEY_BYTES]).confirms(&pass, "abc123", NOW));
    for altered in [
        later,
        swapped,
        format!("{pass}."),
        format!(".{pass}"),
        format!("{member}.0{lapses}.{tag}"),
        format!("{member}.+{lapses}.{tag}"),
        format!("{member}.{lapses}.{tag}="),
        pass.replace('.', ""),
        String::new(),
    ] {
        assert!(!key().confirms(&altered, "abc123", NOW), "{altered}");
    }
}

#[test]
fn a_pass_names_the_member_and_the_moment_it_lapses_and_nothing_of_the_key() {
    let pass = key().issue("abc123", "member-1", NOW);
    let (member, lapses, _) = parts(&pass);

    assert_eq!(pass.split('.').count(), 3);
    assert_eq!(member, "bWVtYmVyLTE");
    assert_eq!(lapses, (NOW + LIFETIME).to_string());
    assert!(!pass.contains(&key().written().trim_end().to_owned()));
}

#[test]
fn a_key_reads_back_as_it_was_written_and_nothing_else_reads_as_one() {
    assert_eq!(Key::read(&key().written()), Ok(key()));
    for text in [
        "",
        "short",
        "not base64!",
        &format!("{}=\n", key().written().trim_end()),
    ] {
        assert!(
            Key::read(text).is_err_and(|refused| refused.file == File::Key.name()),
            "{text:?}"
        );
    }
    assert_eq!(format!("{:?}", key()), "Key(..)");
}

#[test]
fn the_live_stream_paths_read_back_as_written_and_another_format_is_refused() {
    let live = Live::of(vec![LivePath {
        pattern: "^/live/(?P<stream>[^/]+)/".to_owned(),
        stream: "^[a-z0-9]+$".to_owned(),
    }]);
    let elsewhere = live.written().replace(
        &format!("\"format\": {FORMAT}"),
        &format!("\"format\": {}", FORMAT + 1),
    );

    assert_eq!(Live::read(&live.written()), Ok(live));
    assert!(Live::read(&elsewhere).is_err_and(|refused| refused.file == File::Live.name()));
    assert!(Live::read("[]").is_err());
}
