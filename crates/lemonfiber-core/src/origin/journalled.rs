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

use crate::error::withheld::is_secret;

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
mod tests;
