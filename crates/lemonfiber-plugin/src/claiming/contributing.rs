//! The rows a plugin adds to registers lemonfiber already runs.
//!
//! Adding is not overriding, and every rule here is that sentence in one of its
//! shapes: the point has to exist, the identity has to be the plugin's own, and
//! neither an identity nor a remedy's subject may be something the bundled set holds.
//!
//! Nothing here holds a list of fields. What a row carries is its point's to say, and
//! the points are published — so a point added later brings its own row with it and
//! these rules go on deciding, which is the whole reason they are published rather
//! than enumerated in first-party source.

use std::collections::{BTreeMap, BTreeSet};

use crate::extension::{self, Occupied};
use crate::schema::{Contribution, Manifest};
use crate::Violation;

use super::{listed, namespaced_with};

/// The rows a plugin adds to registers lemonfiber already runs.
///
/// Adding is not overriding, and every rule here is that sentence in one of its
/// shapes: the point has to exist, the identity has to be the plugin's own, and
/// neither an identity nor a remedy's subject may be something bundled.
pub(super) fn contributed(manifest: &Manifest, occupied: &[&str], found: &mut Vec<Violation>) {
    if manifest.contributions.is_empty() {
        return;
    }
    let published = extension::published(occupied);
    let points = listed(published.points.iter().map(|point| point.name));
    let asked: BTreeSet<&str> = manifest
        .requires
        .iter()
        .flat_map(|requires| &requires.capabilities)
        .map(String::as_str)
        .collect();

    let mut identities: BTreeSet<&str> = BTreeSet::new();
    let mut checks: BTreeSet<&str> = BTreeSet::new();
    let mut remedied: BTreeSet<&str> = BTreeSet::new();
    // Once per capability rather than once per row. A manifest contributing six checks
    // has one thing to fix, and six copies of a sentence is a list nobody reads to the
    // end of — which is where the rows it also has to fix are.
    let mut unasked: BTreeSet<&str> = BTreeSet::new();

    for entry in &manifest.contributions {
        let at = format!("contribution {}", entry.id);
        if !namespaced_with(&entry.id, &manifest.plugin.id) {
            found.push(Violation {
                location: format!("{at}.id"),
                message: format!(
                    "{} is not namespaced with this plugin's id ({}:…); a contribution is the \
                     plugin's and is attributed to it wherever it appears",
                    entry.id, manifest.plugin.id
                ),
            });
        }
        if !identities.insert(&entry.id) {
            found.push(Violation {
                location: at.clone(),
                message: format!("{} is declared twice", entry.id),
            });
        }
        let Some(point) = published.points.iter().find(|point| point.name == entry.at) else {
            found.push(Violation {
                location: format!("{at}.at"),
                message: format!(
                    "{} names no extension point this build publishes; it publishes: {points}",
                    entry.at
                ),
            });
            continue;
        };
        if !asked.contains(point.requires) {
            unasked.insert(point.requires);
        }
        bundled(entry, occupied, &at, found);
        row(entry, point, &at, found);

        // Asked of the published point rather than of a name written here. A rule that
        // recognised its subject by a literal would go on passing after a rename, with
        // nothing left in the set it was checking.
        if entry.at == extension::check() {
            checks.insert(&entry.id);
        }
        if let Some(about) = &entry.about {
            remedied.insert(about);
        }
    }

    for entry in &manifest.contributions {
        let Some(about) = &entry.about else { continue };
        if !checks.contains(about.as_str()) {
            found.push(Violation {
                location: format!("contribution {}.for", entry.id),
                message: format!(
                    "{about} names no check this manifest declares; a remedy for a check somebody \
                     else's plugin or the bundled set holds would be this plugin editing what \
                     lemonfiber says about a service it did not install"
                ),
            });
        }
    }

    for capability in &unasked {
        found.push(Violation {
            location: "requires.capabilities".to_owned(),
            message: format!(
                "a manifest contributing at one of this build's points asks for {capability} by \
                 name, so a lemonfiber that does not take contributions there refuses the \
                 manifest rather than reading the rows and dropping them"
            ),
        });
    }

    for check in checks.difference(&remedied) {
        found.push(Violation {
            location: format!("contribution {check}"),
            message:
                "carries no remedy; a check that can say something is wrong and nothing about \
                      what to do has moved the work to the operator rather than done it"
                    .to_owned(),
        });
    }
}

/// An identity a bundled row already holds, named wherever a contribution takes it.
///
/// Both the row's own identity and the check a remedy is for, because a remedy
/// pointing at a bundled check is a plugin replacing what lemonfiber says to do about
/// its own finding — which is the same collision wearing the other field's name.
///
/// A bundled identity ending in a dot is a family whose members are the operator's
/// configuration rather than this build's, so everything under it is taken too.
fn bundled(entry: &Contribution, occupied: &[&str], at: &str, found: &mut Vec<Violation>) {
    for (field, named) in [("id", Some(&entry.id)), ("for", entry.about.as_ref())] {
        let Some(named) = named else { continue };
        let Some(held) = occupied.iter().find(|held| {
            **held == named.as_str() || (held.ends_with('.') && named.starts_with(**held))
        }) else {
            continue;
        };
        found.push(Violation {
            location: format!("{at}.{field}"),
            message: format!(
                "{named} is the identity a bundled row already holds ({held}); adding is not \
                 overriding, and standing in for something bundled is an operator's recorded \
                 choice rather than a manifest's assertion"
            ),
        });
    }
}

/// One row, against the shape its own point publishes.
///
/// Nothing here holds a list of fields. What a row carries is the point's to say, so a
/// point added later brings its own row with it and this goes on deciding — which is
/// the whole reason the points are published rather than enumerated in source.
fn row(entry: &Contribution, point: &Occupied, at: &str, found: &mut Vec<Violation>) {
    let takes: BTreeSet<&str> = point
        .row
        .required
        .iter()
        .chain(point.row.optional)
        .copied()
        .collect();
    let carried = present(entry);

    for field in point.row.required {
        if !carried.contains_key(field) {
            found.push(Violation {
                location: at.to_owned(),
                message: format!("carries no {field}, which {} requires", point.name),
            });
        }
    }
    for field in carried.keys() {
        if !takes.contains(field) {
            found.push(Violation {
                location: format!("{at}.{field}"),
                message: format!(
                    "{field} is outside what {} declares; it takes: {}",
                    point.name,
                    listed(takes.iter().copied())
                ),
            });
        }
    }
    for closed in point.row.enums {
        let Some(Held::Word(said)) = carried.get(closed.field) else {
            continue;
        };
        if !closed.values.contains(&said.as_str()) {
            found.push(Violation {
                location: format!("{at}.{}", closed.field),
                message: format!(
                    "{said} is not one {} recognises; it takes: {}",
                    closed.field,
                    listed(closed.values.iter().copied())
                ),
            });
        }
    }
    for bound in point.row.bounds {
        let Some(Held::Number(number)) = carried.get(bound.field) else {
            continue;
        };
        if *number < bound.limits.min || *number > bound.limits.max {
            found.push(Violation {
                location: format!("{at}.{}", bound.field),
                message: format!(
                    "{number} is outside the bounds {} sets: {} to {}",
                    point.name, bound.limits.min, bound.limits.max
                ),
            });
        }
    }
}

/// What one field of a row holds, in the two kinds a published point constrains.
///
/// A closed set is about words and a bound is about numbers, so a row's fields are
/// read back as one or the other and everything else is present without being either.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Held {
    /// A word, which is what a closed set draws from.
    Word(String),
    /// A number, which is what a bound is on.
    Number(u32),
    /// Something the published constraints say nothing about.
    Other,
}

/// Every field this contribution actually declares, by the name the manifest spells.
///
/// The one table in this module, and it exists because the type is a union of every
/// point's row: what a *row* may carry is the point's to say, and what a
/// *contribution* may carry at all is the format's. A field added to the type and left
/// off here would be carried into a register without any point having declared it, so
/// a test below holds the two to each other.
pub(super) fn present(entry: &Contribution) -> BTreeMap<&'static str, Held> {
    let mut carried: BTreeMap<&'static str, Held> = BTreeMap::new();
    carried.insert("id", Held::Word(entry.id.clone()));
    for (field, word) in [
        ("title", &entry.title),
        ("category", &entry.category),
        ("why", &entry.why),
        ("fixture", &entry.fixture),
        ("service", &entry.service),
        ("for", &entry.about),
        ("action", &entry.action),
        ("detail", &entry.detail),
    ] {
        if let Some(word) = word {
            carried.insert(field, Held::Word(word.clone()));
        }
    }
    if let Some(seconds) = entry.timeout_s {
        carried.insert("timeout_s", Held::Number(seconds));
    }
    if entry.request.is_some() {
        carried.insert("request", Held::Other);
    }
    if entry.expect.is_some() {
        carried.insert("expect", Held::Other);
    }
    carried
}
