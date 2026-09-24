//! Where a setting came from when a plugin's change is what put it there.
//!
//! Read from the journal, which is the one record that says which operation wrote a
//! setting and what it held before. A plugin's changes go into it under the plugin's
//! own name, so the last change to a setting names who is answerable for what is in
//! force — where what is in force is still what that change wrote. A setting edited
//! since is somebody else's, and is left to the ordinary answer.
//!
//! **Nothing in this build writes a setting under a plugin's name yet.** Recipes are
//! what would, and none apply yet. So on a real machine today neither answer here
//! is ever given; what establishes that they are right is a journal written by hand in
//! the tests, the same shape an install writes. The day recipes apply, these answers
//! appear with no change here.
//!
//! **Which operations are plugins is read from the journal too.** An install records
//! the Compose document it writes for the plugin, under the plugin's name, at a path
//! named after it — so an operation that made its own document is a plugin. A name
//! list kept here would drift from the operations lemonfiber actually runs.

use std::collections::BTreeSet;
use std::path::Path;

use lemonfiber_ports::withheld::is_secret;

use super::{Origin, Replaced};
use crate::journal::{is_sealed, Change, Kind};

/// Where the directory holding a plugin's Compose document sits beneath the stack.
const DOCUMENTS: &str = "compose/plugins";

/// The operation a reversal records its own changes under.
const UNDO: &str = crate::app::putting_back::OPERATION;

/// Where the setting `key`, now holding `holds`, came from — where a plugin's change is
/// the last thing that wrote it and it still holds what that change wrote.
///
/// Nothing otherwise, which leaves the ordinary answer to be given. `installed` is the
/// record of what is installed, or nothing where it would not read: a plugin missing
/// from a record that was read is gone, and one missing from a record that was not is
/// not known to be.
#[must_use]
pub fn of_journalled(
    key: &str,
    holds: &str,
    changes: &[Change],
    installed: Option<&[String]>,
) -> Option<Origin> {
    let plugins = plugins(changes);
    let sets: Vec<(&str, Option<&str>, &str)> = changes
        .iter()
        .filter_map(|change| match &change.kind {
            Kind::Set {
                key: set,
                previous,
                current,
            } if set == key => Some((
                change.operation.as_str(),
                previous.as_deref(),
                current.as_str(),
            )),
            _ => None,
        })
        .collect();
    let (last, _) = sets.split_last()?;
    let &(named, _, current) = last;
    if !plugins.contains(named) {
        return None;
    }
    if is_sealed(current) {
        return Some(Origin::Unknown {
            why: format!(
                "the plugin {named} set it, and the record of what it wrote is sealed under a \
                 key this machine no longer has"
            ),
        });
    }
    if current != holds {
        return None;
    }
    let Some(installed) = installed else {
        return Some(Origin::Unknown {
            why: format!(
                "the plugin {named} set it, and the record of what is installed will not \
                 read, so whether it still is cannot be told"
            ),
        });
    };
    if !installed.iter().any(|one| one == named) {
        return Some(Origin::Orphaned {
            named: named.to_owned(),
        });
    }
    Some(Origin::Overridden {
        named: named.to_owned(),
        replaced: replaced(key, named, last, &sets, &plugins),
    })
}

/// What the plugin's change replaced, and where that came from.
///
/// Read back past every write the same plugin made to it, so a plugin that set a value
/// twice is said to have replaced what was there before its first write rather than
/// its own earlier one.
fn replaced(
    key: &str,
    named: &str,
    last: &(&str, Option<&str>, &str),
    sets: &[(&str, Option<&str>, &str)],
    plugins: &BTreeSet<String>,
) -> Replaced {
    let run = sets.iter().rev().take_while(|one| one.0 == named);
    let chain = run.clone().count();
    let &(_, previous, _) = run.fold(last, |_, one| one);
    let before = sets
        .get(..sets.len().saturating_sub(chain))
        .unwrap_or_default();
    let withheld = is_secret(key);
    let Some(previous) = previous else {
        // Nothing was in the file, and a setting that is absent reads as this build's
        // default — so the default is what was in force.
        return Replaced {
            value: None,
            withheld: false,
            from: Box::new(Origin::Bundled),
        };
    };
    let shown = (!withheld && !is_sealed(previous)).then(|| previous.to_owned());
    Replaced {
        value: shown,
        withheld: withheld || is_sealed(previous),
        from: Box::new(wrote(previous, before, plugins)),
    }
}

/// Where the value `previous` came from, given every write to the setting before it.
fn wrote(
    previous: &str,
    before: &[(&str, Option<&str>, &str)],
    plugins: &BTreeSet<String>,
) -> Origin {
    if is_sealed(previous) {
        return Origin::Unknown {
            why: "the value it replaced is sealed under a key this machine no longer has"
                .to_owned(),
        };
    }
    let Some(&(operation, _, current)) = before.last() else {
        return Origin::Unknown {
            why: "nothing lemonfiber recorded wrote the value it replaced".to_owned(),
        };
    };
    if current != previous {
        return Origin::Unknown {
            why: "the value it replaced was changed after the last write lemonfiber recorded"
                .to_owned(),
        };
    }
    if plugins.contains(operation) {
        return Origin::Plugin {
            named: operation.to_owned(),
        };
    }
    if operation == UNDO {
        return Origin::Unknown {
            why: "the value it replaced was put back by an undo, and is whatever that undo \
                  restored"
                .to_owned(),
        };
    }
    // Every other operation that writes the settings file writes an answer the operator
    // gave, a choice they made or a repair they accepted — which is the reading the
    // ordinary answer gives a recorded setting too.
    Origin::Operator
}

/// Every operation in the journal that is a plugin: one that recorded making its own
/// Compose document.
fn plugins(changes: &[Change]) -> BTreeSet<String> {
    changes
        .iter()
        .filter(|change| match &change.kind {
            Kind::Made { path } => Path::new(path)
                .ends_with(Path::new(DOCUMENTS).join(format!("{}.yml", change.operation))),
            _ => false,
        })
        .map(|change| change.operation.clone())
        .collect()
}

#[cfg(test)]
mod tests {
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
}
