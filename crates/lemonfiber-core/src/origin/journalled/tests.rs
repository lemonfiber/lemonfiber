use super::of_journalled;
use crate::journal::{Change, Kind};
use crate::origin::{Origin, Replaced};

/// The document an install of `plugin` records making, which is what marks the
/// operation as a plugin's.
fn installed(plugin: &str) -> Change {
    Change {
        at: "1".to_owned(),
        operation: plugin.to_owned(),
        target: "document".to_owned(),
        kind: Kind::Made {
            path: format!("/stack/compose/plugins/{plugin}.yml"),
        },
    }
}

fn set(operation: &str, key: &str, previous: Option<&str>, current: &str) -> Change {
    Change {
        at: "2".to_owned(),
        operation: operation.to_owned(),
        target: ".env".to_owned(),
        kind: Kind::Set {
            key: key.to_owned(),
            previous: previous.map(str::to_owned),
            current: current.to_owned(),
        },
    }
}

fn komga() -> Vec<String> {
    vec!["komga".to_owned()]
}

fn replaced(value: Option<&str>, withheld: bool, from: Origin) -> Replaced {
    Replaced {
        value: value.map(str::to_owned),
        withheld,
        from: Box::new(from),
    }
}

fn overridden(replaced: Replaced) -> Origin {
    Origin::Overridden {
        named: "komga".to_owned(),
        replaced,
    }
}

/// Over a value the operator answered for: both are exposed, and the one replaced
/// is the operator's rather than called a default.
#[test]
fn a_value_set_over_the_operators_answer_carries_both_and_says_whose_the_first_was() {
    let journal = [
        set("setup", "TZ", None, "Europe/Amsterdam"),
        installed("komga"),
        set("komga", "TZ", Some("Europe/Amsterdam"), "UTC"),
    ];

    assert_eq!(
        of_journalled("TZ", "UTC", &journal, Some(&komga())),
        Some(overridden(replaced(
            Some("Europe/Amsterdam"),
            false,
            Origin::Operator
        )))
    );
}

/// Over nothing at all, which is this build's default having been in force.
#[test]
fn a_value_set_where_nothing_was_replaced_the_default() {
    let journal = [installed("komga"), set("komga", "TZ", None, "UTC")];

    assert_eq!(
        of_journalled("TZ", "UTC", &journal, Some(&komga())),
        Some(overridden(replaced(None, false, Origin::Bundled)))
    );
}

/// Over another plugin's value, named.
#[test]
fn a_value_set_over_another_plugins_names_that_plugin() {
    let journal = [
        installed("plex"),
        set("plex", "TZ", None, "Asia/Tokyo"),
        installed("komga"),
        set("komga", "TZ", Some("Asia/Tokyo"), "UTC"),
    ];

    assert_eq!(
        of_journalled("TZ", "UTC", &journal, Some(&komga())),
        Some(overridden(replaced(
            Some("Asia/Tokyo"),
            false,
            Origin::Plugin {
                named: "plex".to_owned()
            }
        )))
    );
}

/// A plugin that wrote it twice replaced what was there before its first write.
#[test]
fn a_plugin_that_wrote_it_twice_replaced_what_was_there_before_the_first() {
    let journal = [
        set("setup", "TZ", None, "Europe/Amsterdam"),
        installed("komga"),
        set("komga", "TZ", Some("Europe/Amsterdam"), "UTC"),
        set("komga", "TZ", Some("UTC"), "Etc/GMT"),
    ];

    assert_eq!(
        of_journalled("TZ", "Etc/GMT", &journal, Some(&komga())),
        Some(overridden(replaced(
            Some("Europe/Amsterdam"),
            false,
            Origin::Operator
        )))
    );
}

/// Where nothing recorded the value it replaced, that is said rather than guessed.
#[test]
fn a_replaced_value_nothing_recorded_is_unknown_and_never_bundled() {
    let journal = [
        installed("komga"),
        set("komga", "TZ", Some("Asia/Tokyo"), "UTC"),
    ];

    let read = of_journalled("TZ", "UTC", &journal, Some(&komga()));

    assert!(
        matches!(&read, Some(Origin::Overridden { replaced, .. }) if matches!(*replaced.from, Origin::Unknown { .. })),
        "{read:?}"
    );
}

/// A value changed by hand between the last recorded write and the plugin's is not
/// attributed to whoever made that last recorded write.
#[test]
fn a_replaced_value_edited_after_its_last_recorded_write_is_unknown() {
    let journal = [
        set("setup", "TZ", None, "Europe/Amsterdam"),
        installed("komga"),
        set("komga", "TZ", Some("Europe/Paris"), "UTC"),
    ];

    let read = of_journalled("TZ", "UTC", &journal, Some(&komga()));

    assert!(
        matches!(&read, Some(Origin::Overridden { replaced, .. }) if replaced.from.why().is_some_and(|why| why.contains("changed after"))),
        "{read:?}"
    );
}

/// A value an undo put back is whatever that undo restored, which is not a thing
/// to name from here.
#[test]
fn a_replaced_value_an_undo_put_back_is_unknown() {
    let journal = [
        set(super::UNDO, "TZ", None, "Europe/Amsterdam"),
        installed("komga"),
        set("komga", "TZ", Some("Europe/Amsterdam"), "UTC"),
    ];

    let read = of_journalled("TZ", "UTC", &journal, Some(&komga()));

    assert!(
        matches!(&read, Some(Origin::Overridden { replaced, .. }) if replaced.from.why().is_some_and(|why| why.contains("undo"))),
        "{read:?}"
    );
}

/// A replaced credential is withheld, and its origin is still said.
#[test]
fn a_replaced_credential_is_withheld_and_never_shown() {
    let journal = [
        set("setup", "INDEXER_APIKEY", None, "the-old-one"),
        installed("komga"),
        set(
            "komga",
            "INDEXER_APIKEY",
            Some("the-old-one"),
            "the-new-one",
        ),
    ];

    assert_eq!(
        of_journalled("INDEXER_APIKEY", "the-new-one", &journal, Some(&komga())),
        Some(overridden(replaced(None, true, Origin::Operator)))
    );
}

/// A replaced value still sealed, because this machine lost the key, is withheld
/// and unknown rather than printed as sealed text.
#[test]
fn a_replaced_value_still_sealed_is_withheld_and_unknown() {
    let journal = [
        installed("komga"),
        set("komga", "TZ", Some("sealed:1:00"), "UTC"),
    ];

    let read = of_journalled("TZ", "UTC", &journal, Some(&komga()));

    assert!(
        matches!(&read, Some(Origin::Overridden { replaced, .. })
            if replaced.value.is_none() && replaced.withheld && !replaced.from.is_settled()),
        "{read:?}"
    );
}

/// What the plugin wrote is itself sealed and will not open: nothing can be said
/// about whether it is still in force, and that is said.
#[test]
fn a_write_whose_record_will_not_open_is_unknown_and_names_the_plugin() {
    let journal = [installed("komga"), set("komga", "TZ", None, "sealed:1:00")];

    let read = of_journalled("TZ", "UTC", &journal, Some(&komga()));

    assert!(
        read.as_ref()
            .and_then(Origin::why)
            .is_some_and(|why| why.contains("komga") && why.contains("sealed")),
        "{read:?}"
    );
}

/// Still in force after the plugin is gone from a record that was read: orphaned,
/// named.
#[test]
fn a_value_a_removed_plugin_left_in_force_is_orphaned_and_named() {
    let journal = [installed("komga"), set("komga", "TZ", None, "UTC")];

    assert_eq!(
        of_journalled("TZ", "UTC", &journal, Some(&[])),
        Some(Origin::Orphaned {
            named: "komga".to_owned()
        })
    );
}

/// A record that would not read cannot say the plugin is gone.
#[test]
fn a_plugin_missing_from_a_record_that_would_not_read_is_not_called_orphaned() {
    let journal = [installed("komga"), set("komga", "TZ", None, "UTC")];

    let read = of_journalled("TZ", "UTC", &journal, None);

    assert!(
        read.as_ref()
            .and_then(Origin::why)
            .is_some_and(|why| why.contains("will not read")),
        "{read:?}"
    );
}

/// Changed since the plugin wrote it, the value is somebody else's and this has
/// nothing to say; nor where the last write was not a plugin's, or nothing wrote it.
#[test]
fn nothing_is_said_where_a_plugin_is_not_what_put_the_value_in_force() {
    let journal = [
        installed("komga"),
        set("komga", "TZ", None, "UTC"),
        set("setup", "PUID", None, "1000"),
    ];

    assert_eq!(
        of_journalled("TZ", "Europe/Paris", &journal, Some(&komga())),
        None
    );
    assert_eq!(
        of_journalled("PUID", "1000", &journal, Some(&komga())),
        None
    );
    assert_eq!(
        of_journalled("PGID", "1000", &journal, Some(&komga())),
        None
    );
}

/// An operation is a plugin's only where it made its own document: a Compose file
/// made under another name, or another path made under its name, is not one.
#[test]
fn only_an_operation_that_made_its_own_document_is_a_plugin() {
    let journal = [
        Change {
            at: "1".to_owned(),
            operation: "apply".to_owned(),
            target: "data".to_owned(),
            kind: Kind::Made {
                path: "/stack/compose/plugins/komga.yml".to_owned(),
            },
        },
        set("apply", "TZ", None, "UTC"),
    ];

    assert_eq!(of_journalled("TZ", "UTC", &journal, Some(&komga())), None);
}
