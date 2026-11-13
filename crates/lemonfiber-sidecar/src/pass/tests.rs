use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;

use super::{
    File, InQuery, ItemPath, Key, LivePath, Paths, Session, FORMAT, KEY_BYTES, LIFETIME,
    NONCE_BYTES,
};

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

    assert!(key().confirms(&pass, "abc123", NOW, &Renewals::none()));
    assert!(key().confirms(&pass, "abc123", NOW + LIFETIME - 1, &Renewals::none()));
    assert!(!key().confirms(&pass, "abc123", NOW + LIFETIME, &Renewals::none()));
    assert!(!key().confirms(&pass, "abc124", NOW, &Renewals::none()));
}

#[test]
fn a_pass_made_with_another_key_or_altered_in_any_part_is_refused() {
    let pass = key().issue("abc123", "member-1", NOW);
    let (member, lapses, tag) = parts(&pass);
    let later = format!("{member}.{}.{tag}", NOW + 10 * LIFETIME);
    let (other_member, _, _) = parts(&key().issue("abc123", "member-2", NOW));
    let swapped = format!("{other_member}.{lapses}.{tag}");

    assert!(!Key::of([8; KEY_BYTES]).confirms(&pass, "abc123", NOW, &Renewals::none()));
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
        assert!(
            !key().confirms(&altered, "abc123", NOW, &Renewals::none()),
            "{altered}"
        );
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
fn the_paths_read_back_as_written_and_another_format_is_refused() {
    let paths = Paths::of(
        vec![
            LivePath {
                pattern: "^/live/(?P<stream>[^/]+)/".to_owned(),
                parameter: None,
                stream: "^[a-z0-9]+$".to_owned(),
            },
            LivePath {
                pattern: "^/videos/[^/]+/live\\.m3u8$".to_owned(),
                parameter: Some("LiveStreamId".to_owned()),
                stream: "^[a-z0-9]+$".to_owned(),
            },
        ],
        vec![ItemPath {
            pattern: "^/photo".to_owned(),
            query: Some(InQuery {
                parameter: "url".to_owned(),
                pattern: "^/library/(?P<item>[0-9]+)$".to_owned(),
            }),
            item: "^[0-9]+$".to_owned(),
        }],
    );
    let elsewhere = paths.written().replace(
        &format!("\"format\": {FORMAT}"),
        &format!("\"format\": {}", FORMAT + 1),
    );

    assert_eq!(Paths::read(&paths.written()), Ok(paths));
    assert!(Paths::read(&elsewhere).is_err_and(|refused| refused.file == File::Paths.name()));
    assert!(Paths::read("[]").is_err());
}

/// The session the item passes below seal.
fn session() -> Session {
    Session {
        member: "member-1".to_owned(),
        token: "ab".repeat(16),
    }
}

/// An item pass for `item`, sealed behind a nonce of no randomness at `NOW`.
fn sealed(item: &str) -> String {
    key()
        .seal(item, &session(), [5; NONCE_BYTES], NOW)
        .unwrap_or_else(|| unreachable!("a session seals"))
}

#[test]
fn an_item_pass_opens_to_its_session_for_its_item_alone_and_says_when_it_lapsed() {
    let pass = sealed("item-1");
    let opened = key().open(&pass, "item-1", NOW, &Renewals::none());

    assert_eq!(
        opened.as_ref().map(|opened| &opened.session),
        Some(&session())
    );
    assert_eq!(opened.map(|opened| opened.lapsed), Some(false));
    assert_eq!(
        key()
            .open(&pass, "item-1", NOW + LIFETIME, &Renewals::none())
            .map(|opened| opened.lapsed),
        Some(true)
    );
    assert_eq!(key().open(&pass, "item-2", NOW, &Renewals::none()), None);
    assert_eq!(
        Key::of([8; KEY_BYTES]).open(&pass, "item-1", NOW, &Renewals::none()),
        None
    );
}

#[test]
fn an_item_pass_altered_in_any_part_does_not_open() {
    let pass = sealed("item-1");
    let (nonce, lapses, sealed) = parts(&pass);
    let other_nonce = URL_SAFE_NO_PAD.encode([6; NONCE_BYTES]);
    let mut flipped = URL_SAFE_NO_PAD.decode(&sealed).unwrap_or_default();
    if let Some(first) = flipped.first_mut() {
        *first ^= 1;
    }
    for altered in [
        format!("{nonce}.{}.{sealed}", NOW + 10 * LIFETIME),
        format!("{other_nonce}.{lapses}.{sealed}"),
        format!("{nonce}.{lapses}.{}", URL_SAFE_NO_PAD.encode(&flipped)),
        format!("{nonce}.0{lapses}.{sealed}"),
        format!("{nonce}.{lapses}.{sealed}."),
        format!("{nonce}.{lapses}"),
        String::new(),
    ] {
        assert_eq!(
            key().open(&altered, "item-1", NOW, &Renewals::none()),
            None,
            "{altered}"
        );
    }
}

#[test]
fn neither_kind_of_pass_is_good_as_the_other() {
    let live = key().issue("item-1", "member-1", NOW);
    let item = sealed("item-1");

    assert_eq!(key().open(&live, "item-1", NOW, &Renewals::none()), None);
    assert!(!key().confirms(&item, "item-1", NOW, &Renewals::none()));
}

#[test]
fn an_item_pass_holds_nothing_readable_and_a_session_says_nothing_of_its_token() {
    let pass = sealed("item-1");

    assert!(!pass.contains(&session().token));
    assert!(!pass.contains(&URL_SAFE_NO_PAD.encode(&session().token)));
    assert!(!format!("{:?}", session()).contains(&session().token));
}

/// What ring's own HKDF-SHA256 draws from `key` under `info`, with no salt.
fn hkdf(key: &Key, info: &'static [u8]) -> [u8; KEY_BYTES] {
    struct Length;
    impl ring::hkdf::KeyType for Length {
        fn len(&self) -> usize {
            KEY_BYTES
        }
    }
    let infos = [info];
    let mut drawn = [0; KEY_BYTES];
    let filled = ring::hkdf::Salt::new(ring::hkdf::HKDF_SHA256, &[])
        .extract(&key.0)
        .expand(&infos, Length)
        .and_then(|okm| okm.fill(&mut drawn));
    assert!(filled.is_ok());
    drawn
}

#[test]
fn each_kind_of_pass_is_made_with_the_key_hkdf_draws_for_it() {
    assert_eq!(
        key().drawn(super::LIVE_INFO),
        hkdf(&key(), super::LIVE_INFO)
    );
    assert_eq!(
        key().drawn(super::ITEM_INFO),
        hkdf(&key(), super::ITEM_INFO)
    );
    assert_ne!(key().drawn(super::LIVE_INFO), key().drawn(super::ITEM_INFO));
}

#[test]
fn a_renewal_holds_the_same_pass_good_to_the_moment_it_names_and_no_other_pass() {
    let live = key().issue("abc123", "member-1", NOW);
    let item = sealed("item-1");
    let mut renewals = Renewals::none();
    for pass in [&live, &item] {
        if let Some(name) = renewed_as(pass) {
            renewals.passes.insert(
                name,
                Renewed {
                    member: "member-1".to_owned(),
                    lapses: NOW + 2 * LIFETIME,
                },
            );
        }
    }
    let later = NOW + LIFETIME + 10;

    assert!(key().confirms(&live, "abc123", later, &renewals));
    assert!(!key().confirms(&live, "abc123", NOW + 2 * LIFETIME, &renewals));
    assert!(!key().confirms(&live, "abc123", later, &Renewals::none()));
    assert!(!key().confirms(
        &key().issue("abc123", "member-2", NOW),
        "abc123",
        later,
        &renewals
    ));
    assert_eq!(
        key()
            .open(&item, "item-1", later, &renewals)
            .map(|opened| opened.lapsed),
        Some(false)
    );
    assert_eq!(key().issued("abc123", "member-1", NOW + LIFETIME), live);
    assert!(renewed_as(&live).is_some_and(|name| !live.contains(&name) && name.len() == 64));
}

#[test]
fn the_renewals_read_back_as_written_and_another_format_is_refused() {
    let mut renewals = Renewals::none();
    renewals.passes.insert(
        "ab".repeat(32),
        Renewed {
            member: "member-1".to_owned(),
            lapses: NOW,
        },
    );
    let elsewhere = renewals.written().replace(
        &format!("\"format\": {FORMAT}"),
        &format!("\"format\": {}", FORMAT + 1),
    );

    assert_eq!(Renewals::read(&renewals.written()), Ok(renewals));
    assert!(Renewals::read(&elsewhere).is_err_and(|refused| refused.file == File::Renewals.name()));
}
