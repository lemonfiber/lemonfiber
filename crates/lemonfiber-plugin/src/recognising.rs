//! Whether the names a plugin manifest uses are ones this build knows.
//!
//! Asked of the untyped tree, before the contract is read into types, for the one
//! reason a typed read cannot answer it: a deserialiser stops at the first word it
//! does not know, and a third-party author then learns their mistakes one run at a
//! time. Here every declaration is asked independently, so the answer is the whole
//! list — which matters more for a plugin than for a stack, because a manifest
//! nobody in this project wrote is likelier to carry several faults at once.
//!
//! Nothing here holds a list of names. Each field is handed to the type that owns
//! the set, and the type's own refusal is what gets reported — which is what keeps
//! this from becoming a second copy of an enumeration, silently disagreeing with
//! the first about what a plugin is allowed to say.

use serde::de::DeserializeOwned;
use toml::Value;

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

/// A declaration whose value has to be a name, and the type that knows the names.
struct Closed {
    /// Where it sits inside one entry, spelled as the manifest spells it.
    at: &'static str,
    /// Asks the type itself, and carries back what it said if the answer is no.
    reads: fn(&Value) -> Option<String>,
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
pub(crate) fn unrecognised(text: &str) -> Vec<Violation> {
    scan(text, DECLARED)
}

/// The same, against a given table, which is what lets the table be tested short.
///
/// Silent on a file that is not TOML at all: there is nothing to walk, and the read
/// that follows describes that failure far better than a scan could.
fn scan(text: &str, declared: &[(&str, &[Closed])]) -> Vec<Violation> {
    toml::from_str::<Value>(text)
        .map(|tree| {
            declared
                .iter()
                .flat_map(|(kind, closed)| entries(&tree, kind, closed))
                .collect()
        })
        .unwrap_or_default()
}

/// Every declared entry of one kind, asked about each of its closed fields.
fn entries(tree: &Value, kind: &str, closed: &[Closed]) -> Vec<Violation> {
    tree.get(kind)
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |declared| {
            declared
                .iter()
                .enumerate()
                .flat_map(|(at, entry)| asked(kind, at, entry, closed))
                .collect()
        })
}

/// One entry's refusals, each naming the field it came from.
fn asked(kind: &str, at: usize, entry: &Value, closed: &[Closed]) -> Vec<Violation> {
    closed
        .iter()
        .filter_map(|field| {
            inside(entry, field.at)
                .and_then(field.reads)
                .map(|said| Violation {
                    location: where_it_is(kind, at, entry),
                    message: format!("{}: {said}", field.at),
                })
        })
        .collect()
}

/// The value at a dotted path inside one entry, where the entry declares it.
fn inside<'a>(entry: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(entry, |here, step| here.get(step))
}

/// Where an entry is, in the manifest's own terms.
///
/// By the id it declares, and by position when it has not declared one — an entry
/// can be missing its id and still say something unrecognised, and "the second
/// service" beats no location at all.
fn where_it_is(kind: &str, at: usize, entry: &Value) -> String {
    entry
        .get("id")
        .and_then(Value::as_str)
        .map_or_else(|| format!("{kind} {at}"), |id| format!("{kind} {id}"))
}

/// What the owning type says about a word, when it does not accept it.
///
/// Only a string is a name. A field holding a number or a table is a type error and
/// a different question from the one asked here — and one the read that follows
/// answers better, with the line it is on.
fn refused<T: DeserializeOwned>(value: &Value) -> Option<String> {
    let word = value.as_str()?;
    Value::from(word)
        .try_into::<T>()
        .err()
        .map(|said| said.to_string().trim_end().to_owned())
}

#[cfg(test)]
mod tests {
    use super::{scan, unrecognised, Closed, Violation, DECLARED, ON_SERVICE};

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
