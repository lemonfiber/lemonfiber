//! Whether the names a manifest uses are ones this build knows.
//!
//! Asked of the untyped tree, before the contract is read into types, for the one
//! reason a typed read cannot answer it: a deserialiser stops at the first word it
//! does not know, and an operator adapting a fork then learns their mistakes one
//! run at a time. Here every declaration is asked independently, so the answer is
//! the whole list.
//!
//! Nothing here holds a list of names. Each field is handed to the type that owns
//! the set, and the type's own refusal is what gets reported — which is what keeps
//! this from becoming a second copy of an enumeration, silently disagreeing with
//! the first about what a service is allowed to say.

use serde::de::DeserializeOwned;
use toml::Value;

use crate::schema::{ApiKind, Bind, Criticality, HealthKind, KeySource, Protocol};
use crate::Violation;

/// A declaration whose value has to be a name, and the type that knows the names.
struct Closed {
    /// Where it sits inside one entry, spelled as the manifest spells it.
    at: &'static str,
    /// Asks the type itself, and carries back what it said if the answer is no.
    reads: fn(&Value) -> Option<String>,
}

/// What a profile declares by name.
const ON_PROFILE: &[Closed] = &[Closed {
    at: "protocol",
    reads: refused::<Protocol>,
}];

/// What a service declares by name, its API and its health probe included.
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
    Closed {
        at: "api.kind",
        reads: refused::<ApiKind>,
    },
    Closed {
        at: "api.key_source",
        reads: refused::<KeySource>,
    },
];

/// Every kind of entry a manifest declares, and what each of them declares by name.
const DECLARED: &[(&str, &[Closed])] = &[("profile", ON_PROFILE), ("service", ON_SERVICE)];

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
/// By the id it declares, matching how validation names things, and by position
/// when it has not declared one — an entry can be missing its id and still say
/// something unrecognised, and "the fourth service" beats no location at all.
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
    use toml::Value;

    use super::{scan, unrecognised, Closed, DECLARED, ON_PROFILE, ON_SERVICE};
    use crate::Manifest;

    /// A word no enumeration in the contract will ever hold.
    const UNKNOWABLE: &str = "zzz-not-a-name";

    #[test]
    fn names_the_field_and_the_entry_it_was_declared_on() {
        let found = unrecognised(
            "
[[service]]
id = \"jellyfin\"
api = { kind = \"plex\", key_source = \"config-xml\" }
",
        );
        let said: Vec<String> = found.iter().map(ToString::to_string).collect();
        assert_eq!(said.len(), 1, "one refusal: {said:?}");
        assert!(
            said.first()
                .is_some_and(|one| one.contains("service jellyfin")
                    && one.contains("api.kind")
                    && one.contains("`plex`")),
            "got: {said:?}"
        );
    }

    #[test]
    fn an_entry_with_no_id_yet_is_still_placed() {
        let found = unrecognised(
            "
[[profile]]
protocol = \"carrier-pigeon\"
",
        );
        assert!(
            found.first().is_some_and(|one| one.location == "profile 0"),
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

    /// Swap the `want`th string in the tree, counting through `seen` as it goes.
    fn nth_word(value: &mut Value, want: usize, seen: &mut usize) {
        match value {
            Value::String(word) => {
                if *seen == want {
                    UNKNOWABLE.clone_into(word);
                }
                *seen += 1;
            }
            Value::Array(items) => items.iter_mut().for_each(|item| nth_word(item, want, seen)),
            Value::Table(fields) => fields
                .iter_mut()
                .for_each(|(_, field)| nth_word(field, want, seen)),
            _ => {}
        }
    }

    /// What the typed read said about one swapped word the scan did not name.
    ///
    /// Asked of the read rather than of `from_toml`, because `from_toml` consults the
    /// whole table and would name what a short one missed — which is the answer this
    /// is trying to tell apart.
    fn escaped_at(tree: &Value, nth: usize, declared: &[(&str, &[Closed])]) -> Option<String> {
        let mut copy = tree.clone();
        nth_word(&mut copy, nth, &mut 0);
        let text = toml::to_string(&copy).ok()?;
        if !scan(&text, declared).is_empty() {
            return None;
        }
        toml::from_str::<Manifest>(&text)
            .err()
            .map(|said| said.to_string())
            .filter(|said| said.contains("unknown variant"))
    }

    /// Every word in a manifest, swapped in turn for one no enumeration holds.
    fn swept(tree: &Value, declared: &[(&str, &[Closed])]) -> (usize, Vec<String>) {
        let mut words = 0;
        nth_word(&mut tree.clone(), usize::MAX, &mut words);
        let escaped = (0..words)
            .filter_map(|nth| escaped_at(tree, nth, declared))
            .collect();
        (words, escaped)
    }

    /// The shipped stack, and what a sweep of it found a given table failing to name.
    fn sweeping(declared: &[(&str, &[Closed])]) -> (usize, Vec<String>) {
        let embedded = include_str!("../../../assets/media-stack/stack.toml");
        toml::from_str::<Value>(embedded)
            .map(|tree| swept(&tree, declared))
            .unwrap_or_default()
    }

    /// The table above is a list of paths, and a list is a thing that goes stale.
    ///
    /// Adding a field whose type is a closed set and forgetting to list it here is
    /// invisible: the manifest still refuses the unknown name, just as the parse
    /// failure this module exists to replace. So every word the shipped stack
    /// declares is swapped in turn for one nothing can accept, and the only answer
    /// allowed is a refusal that named it — never one that failed to parse.
    #[test]
    fn no_closed_field_in_the_shipped_stack_is_left_off_the_table() {
        let (words, escaped) = sweeping(DECLARED);
        assert!(words > 100, "the shipped stack was read and swept: {words}");
        assert!(
            escaped.is_empty(),
            "a closed field is missing from the table above, so an unknown name \
             came back as a parse failure: {escaped:?}"
        );
    }

    /// And the sweep can fail, which is the half a passing sweep says nothing about.
    ///
    /// A gate nobody has watched refuse is a gate nobody knows works: run against a
    /// table with the last service field taken off it, every service declaring that
    /// field has to come back unnamed.
    #[test]
    fn a_field_left_off_the_table_is_what_the_sweep_refuses() {
        let short = ON_SERVICE.split_last().map_or(&[][..], |(_, rest)| rest);
        let (_, escaped) = sweeping(&[("profile", ON_PROFILE), ("service", short)]);
        assert!(
            escaped.iter().any(|said| said.contains("key_source")),
            "a table missing api.key_source lets it through as a parse failure: \
             {escaped:?}"
        );
    }
}
