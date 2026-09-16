//! Which of `plugin.toml`'s declarations are names, and which type owns each set.
//!
//! The table and nothing else. How a table is walked, and how a refusal is placed in
//! the manifest's own terms, is [`lemonfiber_manifest::names`] — the same walk the
//! stack manifest uses, asked here of a different set of fields.
//!
//! Sharing it matters more than it looks. The point of a names-before-types read is
//! that an author is told every mistake in one run instead of one per run, and a
//! third-party author needs that more than anybody adapting a fork does — so the two
//! manifests answering in the same words, and placing a faulty entry the same way, is
//! the whole of what makes the read worth having.
//!
//! Nothing here holds a list of names. Each field is handed to the type that owns the
//! set, and the type's own refusal is what gets reported — which is what keeps this
//! from becoming a second copy of an enumeration, silently disagreeing with the first
//! about what a plugin is allowed to say.

use lemonfiber_manifest::names::{refused, scan, Closed};

use crate::schema::{Bind, Criticality, HealthKind};

/// One thing a manifest got wrong, and where.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Violation {
    /// Which declaration it is about, in the manifest's own terms.
    pub location: String,
    /// What is wrong with it.
    pub message: String,
}

impl std::fmt::Display for Violation {
    fn fmt(&self, into: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(into, "{}: {}", self.location, self.message)
    }
}

/// What a plugin's service declares by name, its health probe included.
const ON_SERVICE: &[Closed] = &[
    Closed {
        at: "bind",
        reads: refused::<Bind>,
    },
    Closed {
        at: "criticality",
        reads: refused::<Criticality>,
    },
    Closed {
        at: "health.kind",
        reads: refused::<HealthKind>,
    },
];

/// Every kind of entry a manifest declares, and what each of them declares by name.
const DECLARED: &[(&str, &[Closed])] = &[("service", ON_SERVICE)];

/// Everything the manifest declares that this build does not recognise.
///
/// The walk answers with a location and a message; what a fault *is* to this crate is
/// this crate's own — it is serialised into a report, which the stack manifest's never
/// is — so the pair becomes a [`Violation`] here rather than there.
pub(crate) fn unrecognised(text: &str) -> Vec<Violation> {
    scan(text, DECLARED)
        .into_iter()
        .map(|refusal| Violation {
            location: refusal.location,
            message: refusal.message,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use lemonfiber_manifest::names::{scan, Closed};

    use super::{unrecognised, Violation, DECLARED, ON_SERVICE};

    #[test]
    fn a_violation_names_where_it_is_before_what_it_is() {
        let said = Violation {
            location: "service komga".to_owned(),
            message: "bind: unknown variant `wan`".to_owned(),
        }
        .to_string();
        assert_eq!(said, "service komga: bind: unknown variant `wan`");
    }

    #[test]
    fn names_the_field_and_the_entry_it_was_declared_on() {
        let found = unrecognised(
            "
[[service]]
id = \"komga\"
bind = \"wan\"
",
        );
        let said: Vec<String> = found.iter().map(ToString::to_string).collect();
        assert_eq!(said.len(), 1, "one refusal: {said:?}");
        assert!(
            said.first().is_some_and(|one| one.contains("service komga")
                && one.contains("bind")
                && one.contains("`wan`")),
            "got: {said:?}"
        );
    }

    /// The criticality a plugin may not declare is refused as a name, not as a rule.
    ///
    /// `critical` means *its failure has consequences outside the machine*, which is
    /// not a severity a contributor assigns to their own work. The four a plugin may
    /// use are what the type carries, so the refusal names the value and lists them
    /// without anything here holding a second copy of the list.
    #[test]
    fn the_criticality_a_plugin_may_not_claim_is_named_with_the_four_it_may() {
        let found = unrecognised(
            "
[[service]]
id = \"komga\"
criticality = \"critical\"
",
        );
        let said = found.first().map(ToString::to_string).unwrap_or_default();
        assert!(said.contains("`critical`"), "names the value: {said}");
        for available in ["core", "important", "enhancing", "optional"] {
            assert!(said.contains(available), "names {available}: {said}");
        }
    }

    #[test]
    fn an_entry_with_no_id_yet_is_still_placed() {
        let found = unrecognised(
            "
[[service]]
health = { kind = \"carrier-pigeon\" }
",
        );
        assert!(
            found.first().is_some_and(|one| one.location == "service 0"),
            "got: {found:?}"
        );
    }

    #[test]
    fn a_field_holding_something_other_than_a_name_is_left_to_the_reader() {
        let found = unrecognised(
            "
[[service]]
id = \"q\"
criticality = 3
",
        );
        assert!(
            found.is_empty(),
            "a number is not a name it refused: {found:?}"
        );
    }

    #[test]
    fn a_file_that_is_not_toml_is_left_to_the_reader() {
        assert!(unrecognised("= not toml").is_empty());
    }

    /// The table above is a list of paths, and a list is a thing that goes stale.
    ///
    /// Run against a table with the last service field taken off it, a manifest
    /// declaring that field has to come back unnamed — which is what a field left off
    /// the table looks like, and the only way to know the sweep can fail at all.
    #[test]
    fn a_field_left_off_the_table_is_not_named() {
        const DECLARES: &str = "
[[service]]
id = \"komga\"
health = { kind = \"carrier-pigeon\" }
";
        let short: &[Closed] = ON_SERVICE.split_last().map_or(&[], |(_, rest)| rest);
        assert!(
            !scan(DECLARES, DECLARED).is_empty(),
            "the whole table names it"
        );
        assert!(
            scan(DECLARES, &[("service", short)]).is_empty(),
            "and a table missing health.kind lets it through"
        );
    }
}
