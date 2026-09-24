use super::{asking, read, NOTHING};
use lemonfiber_ports::docker::Presence;

/// The two answers the whole check is built on, in the daemon's own words.
#[test]
fn a_refused_mount_and_a_missing_image_are_the_two_halves_of_the_answer() {
    assert_eq!(
        read(
            400,
            r#"invalid mount config for type "bind": bind source path does not exist: /srv/media"#
        ),
        Presence::Absent
    );
    assert_eq!(read(404, "No such image: sha256:0000"), Presence::There);
}

/// A refusal about something other than the source is not an answer about it.
///
/// The dangerous reading is the other way round: a 400 taken as "absent" whatever
/// it said would refuse a machine that has the path, over a mount option.
#[test]
fn a_refusal_that_is_not_about_the_source_says_nothing_about_the_source() {
    assert_eq!(
        read(
            400,
            "invalid mount config for type \"bind\": invalid mount path"
        ),
        Presence::Unknown
    );
    assert_eq!(read(500, "server error"), Presence::Unknown);
    assert_eq!(read(409, "conflict"), Presence::Unknown);
}

/// Wording drifts, and a reading that no longer recognises a "no" must say so
/// rather than quietly stop finding any.
#[test]
fn a_refusal_worded_some_other_way_is_not_read_as_a_yes() {
    assert_eq!(read(400, "bind mount source missing"), Presence::Unknown);
}

/// Nothing in the request can name something a machine might actually have, and
/// the path goes in as data rather than as anything a shell would read.
#[test]
fn the_request_names_an_image_nothing_can_have_and_carries_the_path_whole() {
    let body = asking("/srv/media; touch /tmp/pwned");
    assert_eq!(body.image.as_deref(), Some(NOTHING));

    let mounts = body
        .host_config
        .and_then(|config| config.mounts)
        .unwrap_or_default();
    assert_eq!(mounts.len(), 1, "{mounts:?}");
    assert_eq!(
        mounts.first().and_then(|mount| mount.source.clone()),
        Some("/srv/media; touch /tmp/pwned".to_owned()),
        "a path is a field in a document here, not a word in a command"
    );
    assert_eq!(mounts.first().and_then(|mount| mount.read_only), Some(true));
}
