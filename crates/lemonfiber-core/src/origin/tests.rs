use super::{of_setting, Origin};
use crate::baseline::{Origin as Recorded, Record};

/// A record of what lemonfiber last wrote, in the origin named.
fn recorded(value: &str, origin: Recorded) -> Record {
    Record {
        value: value.to_owned(),
        at: "0".to_owned(),
        origin,
    }
}

/// The two states a plugin's own change leaves have a word, and name the plugin.
#[test]
fn an_overridden_and_an_orphaned_value_have_a_word_and_name_their_plugin() {
    let overridden = Origin::Overridden {
        named: "komga".to_owned(),
        replaced: super::Replaced {
            value: None,
            withheld: false,
            from: Box::new(Origin::Bundled),
        },
    };
    let orphaned = Origin::Orphaned {
        named: "plex".to_owned(),
    };

    assert_eq!(overridden.as_str(), "overridden");
    assert_eq!(orphaned.as_str(), "orphaned");
    assert_eq!(overridden.plugin(), Some("komga"));
    assert_eq!(orphaned.plugin(), Some("plex"));
}

#[test]
fn each_origin_has_the_word_a_report_uses_for_it() {
    assert_eq!(Origin::Bundled.as_str(), "bundled");
    assert_eq!(Origin::Operator.as_str(), "operator");
    assert_eq!(
        Origin::Plugin {
            named: "komga".to_owned()
        }
        .as_str(),
        "plugin"
    );
    assert_eq!(
        Origin::Unknown {
            why: "no record".to_owned()
        }
        .as_str(),
        "unknown"
    );
}

/// Unknown is the one answer that establishes nothing, and the three that do are
/// told apart from it by being read rather than by each surface deciding again.
#[test]
fn only_an_unknown_origin_is_unsettled() {
    assert!(Origin::Bundled.is_settled());
    assert!(Origin::Operator.is_settled());
    assert!(Origin::Plugin {
        named: "komga".to_owned()
    }
    .is_settled());
    assert!(!Origin::Unknown {
        why: "no record".to_owned()
    }
    .is_settled());
}

/// Only the unsettled answer carries a reason, and the reason is what it was
/// given — a settled origin has nothing to explain and says nothing.
#[test]
fn only_an_unknown_origin_says_why() {
    assert_eq!(
        Origin::Unknown {
            why: "no record".to_owned()
        }
        .why(),
        Some("no record")
    );
    assert_eq!(Origin::Bundled.why(), None);
    assert_eq!(Origin::Operator.why(), None);
    assert_eq!(
        Origin::Plugin {
            named: "komga".to_owned()
        }
        .why(),
        None
    );
}

#[test]
fn only_a_plugin_origin_names_a_plugin() {
    assert_eq!(
        Origin::Plugin {
            named: "komga".to_owned()
        }
        .plugin(),
        Some("komga")
    );
    assert_eq!(Origin::Bundled.plugin(), None);
    assert_eq!(Origin::Operator.plugin(), None);
    assert_eq!(
        Origin::Unknown {
            why: "no record".to_owned()
        }
        .plugin(),
        None
    );
}

/// A value lemonfiber wrote is one the operator asked it to write: the defaults
/// this build carries never reach the file, so a line that is there answers a
/// question somebody was asked.
#[test]
fn a_setting_lemonfiber_has_a_record_for_is_the_operators() {
    assert_eq!(
        of_setting(
            "DATA_ROOT",
            Some(&recorded("/srv/media", Recorded::Written))
        ),
        Origin::Operator
    );
}

/// Adoption is the other way a value becomes the operator's, and it answers the
/// same way: the distinction the baseline keeps is about whose value to re-assert,
/// not about who settled it.
#[test]
fn a_value_lemonfiber_adopted_from_the_operator_is_theirs_too() {
    assert_eq!(
        of_setting(
            "DATA_ROOT",
            Some(&recorded("/srv/media", Recorded::Adopted))
        ),
        Origin::Operator
    );
}

/// The answer a machine with no baseline gives about every one of its settings,
/// and it is the floor rather than a guess at what setup would have written.
#[test]
fn a_setting_with_no_record_is_unknown_rather_than_bundled() {
    let held = of_setting("DATA_ROOT", None);
    assert!(!held.is_settled());
    assert_ne!(held, Origin::Bundled);
    assert!(
        held.why()
            .is_some_and(|why| why.contains("no record of writing here")),
        "the reason names what was missing: {held:?}"
    );
}

/// A credential is left out of the record on purpose, so its origin is unknown on
/// every machine — and the reason says the rule worked rather than that something
/// was lost.
#[test]
fn a_credential_says_it_is_unknown_because_it_is_never_recorded() {
    let held = of_setting("PROVIDER_PASS", None);
    assert_ne!(held, Origin::Bundled);
    assert!(
        held.why()
            .is_some_and(|why| why.contains("keeps no record of a credential")),
        "the reason names the rule rather than a loss: {held:?}"
    );
}
