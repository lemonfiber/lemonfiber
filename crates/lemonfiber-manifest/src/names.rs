//! Asking each declaration's own type about the word it was given.
//!
//! The machinery behind a names-before-types read, and nothing about which names
//! there are. A manifest hands over a table of *where a name sits* and *the type that
//! owns the set*, and gets back every word that type would not accept, each placed in
//! the manifest's own terms.
//!
//! It is here rather than beside either table because there are two manifests.
//! `stack.toml` and `plugin.toml` are different contracts with different closed
//! fields, and they were reading them with the same seventy lines written twice. Two
//! copies of a walk is two places for the placement to drift — for "the fourth
//! service" to become "service 4" in one of them and not the other — and neither
//! crate's tests could see it, because each copy is self-consistent. The whole point
//! of this read is that an author is told every mistake at once, in words that match
//! the file they wrote.
//!
//! Nothing here holds a list of names. That is each manifest's own business, and
//! keeping it there is what stops this becoming a second copy of an enumeration,
//! silently disagreeing with the first about what a declaration is allowed to say.
//!
//! Nor does it hold either crate's idea of what a fault *is*. Both call one a
//! violation and each has its own reasons for the shape of theirs, so what comes back
//! is a location and a message, and each crate builds its own from the pair.

use serde::de::DeserializeOwned;
use toml::Value;

/// One word a manifest used that the type owning the set would not accept.
///
/// Deliberately not either crate's `Violation`. This module knows where a fault is
/// and what is wrong with it, and nothing about what the manifest reading it will do
/// with that — which is the difference between sharing a walk and merging two
/// contracts that happen to look alike today.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// Which declaration it is about, in the manifest's own terms.
    pub location: String,
    /// What is wrong with it.
    pub message: String,
}

/// A declaration whose value has to be a name, and the type that knows the names.
pub struct Closed {
    /// Where it sits inside one entry, spelled as the manifest spells it.
    pub at: &'static str,
    /// Asks the type itself, and carries back what it said if the answer is no.
    pub reads: fn(&Value) -> Option<String>,
}

/// Everything a manifest declares, against a table of what each entry may name.
///
/// Silent on a file that is not TOML at all: there is nothing to walk, and the read
/// that follows describes that failure far better than a scan could.
#[must_use]
pub fn scan(text: &str, declared: &[(&str, &[Closed])]) -> Vec<Refusal> {
    toml::from_str::<Value>(text)
        .map(|tree| {
            declared
                .iter()
                .flat_map(|(kind, closed)| entries(&tree, kind, closed))
                .collect()
        })
        .unwrap_or_default()
}

/// What the owning type says about a word, when it does not accept it.
///
/// Only a string is a name. A field holding a number or a table is a type error and a
/// different question from the one asked here — and one the read that follows answers
/// better, with the line it is on.
#[must_use]
pub fn refused<T: DeserializeOwned>(value: &Value) -> Option<String> {
    let word = value.as_str()?;
    Value::from(word)
        .try_into::<T>()
        .err()
        .map(|said| said.to_string().trim_end().to_owned())
}

/// Every declared entry of one kind, asked about each of its closed fields.
fn entries(tree: &Value, kind: &str, closed: &[Closed]) -> Vec<Refusal> {
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
fn asked(kind: &str, at: usize, entry: &Value, closed: &[Closed]) -> Vec<Refusal> {
    closed
        .iter()
        .filter_map(|field| {
            inside(entry, field.at)
                .and_then(field.reads)
                .map(|said| Refusal {
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
/// By the id it declares, matching how validation names things, and by position when
/// it has not declared one — an entry can be missing its id and still say something
/// unrecognised, and "the fourth service" beats no location at all.
fn where_it_is(kind: &str, at: usize, entry: &Value) -> String {
    entry
        .get("id")
        .and_then(Value::as_str)
        .map_or_else(|| format!("{kind} {at}"), |id| format!("{kind} {id}"))
}
