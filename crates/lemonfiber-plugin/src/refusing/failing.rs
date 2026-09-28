//! Declarations that an assertion fails on a recording, held to the assertion they
//! are about.
//!
//! A declaration excuses one failure: the recording it names, failing on the constraint
//! it names. So the constraint has to be one the assertion makes, the place has to be
//! one that constraint looks at, and the reason has to be there, because it is reported
//! with the verdict every time. A declaration that named something the assertion does
//! not ask would be excusing a failure that cannot happen, and one with no reason would
//! be excusing one nobody has explained.
//!
//! Whether the recording exists is answered where the recordings are, which is not
//! here: this reads a manifest and has no directory to look in.

use std::collections::BTreeSet;

use super::evidence::places;
use crate::claiming::{carries, listed};
use crate::schema::{Declaration, Expect, Manifest};
use crate::vocabulary::Constraint;
use crate::Violation;

/// Every declaration in a manifest, refused wherever it does not describe its assertion.
pub(super) fn declared(manifest: &Manifest, found: &mut Vec<Violation>) {
    for proof in &manifest.proofs {
        within(
            &format!("proof {}", proof.id),
            Some(&proof.expect),
            &proof.expected,
            found,
        );
    }
    for entry in &manifest.contributions {
        within(
            &format!("contribution {}", entry.id),
            entry.expect.as_ref(),
            &entry.expected,
            found,
        );
    }
}

/// One assertion's declarations.
///
/// An assertion with no expectation has nothing for a declaration to name, and is
/// refused for that by the point it is declared at rather than here.
fn within(at: &str, expect: Option<&Expect>, expected: &[Declaration], found: &mut Vec<Violation>) {
    let mut named = BTreeSet::new();
    for declaration in expected {
        let place = format!("{at}.expected {}", declaration.fixture);
        if declaration.fixture.trim().is_empty() {
            found.push(Violation {
                location: format!("{at}.expected"),
                message: "names no recording, so there is nothing for the assertion to fail on"
                    .to_owned(),
            });
        } else if !named.insert(declaration.fixture.as_str()) {
            found.push(Violation {
                location: place.clone(),
                message: "is named by more than one declaration; a recording is declared to \
                          fail once, on the one constraint that fails there"
                    .to_owned(),
            });
        }
        if declaration.reason.trim().is_empty() {
            found.push(Violation {
                location: format!("{place}.reason"),
                message: "is blank; the reason is reported with the verdict every time, and a \
                          declaration without one excuses a failure nobody has explained"
                    .to_owned(),
            });
        }
        if let Some(expect) = expect {
            constrained(&place, expect, declaration, found);
        }
    }
}

/// Whether the constraint a declaration names is one its assertion makes, at the place it
/// names.
fn constrained(at: &str, expect: &Expect, declaration: &Declaration, found: &mut Vec<Violation>) {
    let constraint = declaration.constraint;
    if !carries(expect, constraint) {
        found.push(Violation {
            location: format!("{at}.constraint"),
            message: format!(
                "{} is not a constraint this assertion's expect makes; it makes {}",
                constraint.as_str(),
                listed(
                    Constraint::EVERY
                        .into_iter()
                        .filter(|one| carries(expect, *one))
                        .map(Constraint::as_str)
                )
            ),
        });
        return;
    }
    let looked_at: Vec<&str> = places(expect)
        .into_iter()
        .filter(|(field, _)| *field == constraint.as_str())
        .map(|(_, key)| key)
        .collect();
    match (&declaration.place, constraint.is_key_wise()) {
        (None, true) => found.push(Violation {
            location: format!("{at}.place"),
            message: format!(
                "is absent; {} constrains places, so a declaration names the one that fails: {}",
                constraint.as_str(),
                listed(looked_at.iter())
            ),
        }),
        (Some(place), true) if !looked_at.contains(&place.as_str()) => found.push(Violation {
            location: format!("{at}.place"),
            message: format!(
                "{place} is not a place {} constrains in this assertion; it constrains {}",
                constraint.as_str(),
                listed(looked_at.iter())
            ),
        }),
        (Some(place), false) => found.push(Violation {
            location: format!("{at}.place"),
            message: format!(
                "{place} is named, and {} is about the answer as a whole, so there is no place \
                 within it to fail",
                constraint.as_str()
            ),
        }),
        _ => {}
    }
}

#[cfg(test)]
mod tests;
