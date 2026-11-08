use super::{Covers, TAG_PREFIX};

#[test]
fn what_a_monitor_covers_is_read_back_from_the_tag_it_was_given() {
    for covers in [
        Covers::Service("sonarr".to_owned()),
        Covers::EntryPoint("jellyfin".to_owned()),
        Covers::Job("guard".to_owned()),
    ] {
        assert!(covers.tag().starts_with(TAG_PREFIX));
        assert_eq!(Covers::read(&covers.tag()), Some(covers));
    }
    assert_eq!(
        Covers::EntryPoint("seerr".to_owned()).tag(),
        "lemonfiber:entry:seerr"
    );
}

#[test]
fn a_tag_that_is_not_lemonfibers_or_names_nothing_covers_nothing() {
    for tag in [
        "",
        "service:sonarr",
        "kuma:service:sonarr",
        "lemonfiber:service:",
        "lemonfiber:service",
        "lemonfiber:volume:data",
    ] {
        assert_eq!(Covers::read(tag), None, "{tag}");
    }
}
