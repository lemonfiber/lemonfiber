//! What a recipe is refused for, without anything running one.
//!
//! The block is in the format before its engine is, and that is the point rather than an
//! awkwardness: a rule that cannot be stated for want of a field is not being enforced,
//! and adding the field later would make every manifest written against this generation
//! wrong. So a recipe is declarable now, and everything about it that can be decided by
//! reading is decided by reading.
//!
//! **Nothing here runs one, and the manifest says so rather than implying it.** Running
//! a recipe is a capability a manifest asks for by name, so a build that does not offer
//! it refuses such a manifest naming that capability. Parsing the block and skipping it
//! would install a plugin whose declared behaviour is wider than its actual one, which is
//! the tolerated unknown the whole format exists to refuse.
//!
//! What is decidable by reading is the set of flows the recipe could produce: every
//! substitution inside a call is one value reaching one destination, and each of them has
//! to be declared. That is what makes "what does this plugin tell whom" a question
//! answerable from the file rather than from a run.

use std::collections::BTreeSet;

use crate::schema::{Manifest, Recipe, Step, RUN};
use crate::Violation;

/// What a substitution is written between.
const OPENS: &str = "{{";

/// And what closes it.
const CLOSES: &str = "}}";

/// The methods a call may use.
///
/// Closed, because a recipe's calls are the riskiest thing in the format and the set of
/// verbs it needs is small and known. A word outside it is one no runner could make.
///
/// Reachable from beside this, where a verb is asked whether it changes what it is
/// sent to. Two lists of verbs would be two answers to what a call may say, and the
/// one that fell behind would be classifying a word the other had just admitted.
pub(super) const METHODS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE"];

/// Everything this build refuses about the recipes a manifest declares.
pub(super) fn declared(manifest: &Manifest, found: &mut Vec<Violation>) {
    if manifest.recipes.is_empty() {
        return;
    }
    asked_for(manifest, found);

    let mut named: BTreeSet<&str> = BTreeSet::new();
    for recipe in &manifest.recipes {
        let at = format!("recipe {}", recipe.id);
        worded(&recipe.id, &format!("{at}.id"), found);
        for (number, pair) in recipe.pairs.iter().enumerate() {
            let here = format!("{at}.pair #{}", number + 1);
            worded(&pair.value, &format!("{here}.value"), found);
            destination(&pair.to, &format!("{here}.to"), found);
        }
        if !named.insert(&recipe.id) {
            found.push(Violation {
                location: at.clone(),
                message: "is declared twice, so naming one of them names both".to_owned(),
            });
        }
        flows(recipe, &at, found);
    }
}

/// Whether the capability that runs a recipe was asked for, and whether it is offered.
///
/// Asked once for the whole manifest rather than once per recipe: a plugin declaring six
/// flows has one thing to fix, and six copies of a sentence is a list nobody reads to the
/// end of.
fn asked_for(manifest: &Manifest, found: &mut Vec<Violation>) {
    let asked = manifest
        .requires
        .iter()
        .flat_map(|requires| &requires.capabilities)
        .any(|name| name == RUN);
    if !asked {
        found.push(Violation {
            location: "requires.capabilities".to_owned(),
            message: format!(
                "a manifest declaring a recipe asks for {RUN} by name, so a lemonfiber that \
                 cannot run one refuses the manifest rather than reading the block and skipping \
                 it"
            ),
        });
    }
}

/// Every value one recipe could carry to every destination it could reach.
///
/// Computed by reading, which is what the shape of a call is for: a substitution is the
/// only way a captured value leaves the step that took it, so the flows are the
/// substitutions and nothing else. A flow with no declared pair behind it fails here,
/// before a call is made.
fn flows(recipe: &Recipe, at: &str, found: &mut Vec<Violation>) {
    let mut steps: BTreeSet<&str> = BTreeSet::new();
    let mut captured: BTreeSet<&str> = BTreeSet::new();

    for step in &recipe.steps {
        let here = format!("{at}.step {}", step.id);
        if !steps.insert(&step.id) {
            found.push(Violation {
                location: here.clone(),
                message: "is declared twice, so a verdict against it names two calls".to_owned(),
            });
        }
        worded(&step.id, &format!("{here}.id"), found);
        calling(step, &here, found);
        substituting(step, recipe, &captured, &here, found);

        for capture in &step.capture {
            worded(&capture.name, &format!("{here}.capture.name"), found);
            if !captured.insert(&capture.name) {
                found.push(Violation {
                    location: format!("{here}.capture"),
                    message: format!(
                        "{} is captured twice, so a later call substituting it could mean either",
                        capture.name
                    ),
                });
            }
        }
    }
}

/// What one step calls, and whether it names something rather than somewhere.
///
/// A destination is a service in this stack or a DNS name outside it, never an address.
/// An address is a machine on the operator's network that the manifest chose, which is a
/// reach nothing in the declaration bounds.
fn calling(step: &Step, at: &str, found: &mut Vec<Violation>) {
    if !METHODS.contains(&step.call.method.as_str()) {
        found.push(Violation {
            location: format!("{at}.call.method"),
            message: format!("{} is not one of: {}", step.call.method, METHODS.join(", ")),
        });
    }
    if !is_name(&step.call.to) {
        destination(&step.call.to, &format!("{at}.call.to"), found);
    } else if looks_like_an_address(&step.call.to) {
        found.push(Violation {
            location: format!("{at}.call.to"),
            message: format!(
                "{} is an address rather than a name; a call names a service in this stack or a \
                 DNS name outside it, so that where it goes is a thing the operator can read",
                step.call.to
            ),
        });
    }
}

/// The longest a host name may be, as DNS bounds it.
const LONGEST_NAME: usize = 253;

/// The longest one label of a host name may be, as DNS bounds it.
const LONGEST_LABEL: usize = 63;

/// Refuse an id or a value that is not one word.
///
/// A recipe, a step and a value are named in what an operator approves and in the
/// account a rehearsal gives, and a name carrying a space, a mark that draws nothing or
/// an instruction to the terminal is one that reads differently from what is matched.
/// One word of ASCII letters, digits, `-` and `_` reads the same everywhere it is
/// printed, and is the same bytes wherever it is matched.
fn worded(text: &str, at: &str, found: &mut Vec<Violation>) {
    let word = !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
    if !word {
        found.push(Violation {
            location: at.to_owned(),
            message: format!(
                "{text:?} is not one word of letters, digits, `-` and `_`, so what an operator \
                 reads and what is matched could differ"
            ),
        });
    }
}

/// Refuse a destination that is not a name.
fn destination(to: &str, at: &str, found: &mut Vec<Violation>) {
    if !is_name(to) {
        found.push(Violation {
            location: at.to_owned(),
            message: format!(
                "{to:?} is not a service id or a host name; a destination is lowercase labels of \
                 letters, digits and `-`, joined by dots, so where a value goes is a thing the \
                 operator can read and approve as it is written"
            ),
        });
    }
}

/// Whether a destination is a name: a service id or a host name, in lowercase labels of
/// letters, digits and hyphens joined by dots, within the lengths DNS allows.
///
/// Lowercase because a name differing only in case is the same host to DNS and a
/// different string to anybody matching an approval against it.
fn is_name(to: &str) -> bool {
    !to.is_empty()
        && to.len() <= LONGEST_NAME
        && to.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= LONGEST_LABEL
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

/// Whether a destination is a numeric address rather than a name.
///
/// Read as "every part between the dots is a number", which is what an address is and
/// what no DNS name can be — the last label of a name is never all digits.
fn looks_like_an_address(to: &str) -> bool {
    let parts: Vec<&str> = to.split('.').collect();
    to.contains(':')
        || (parts.len() > 1
            && parts
                .iter()
                .all(|part| !part.is_empty() && part.chars().all(|one| one.is_ascii_digit())))
}

/// Every value this step carries, against what was captured and what was declared.
fn substituting(
    step: &Step,
    recipe: &Recipe,
    captured: &BTreeSet<&str>,
    at: &str,
    found: &mut Vec<Violation>,
) {
    let mut carried: Vec<(String, &str)> = Vec::new();
    if let Some(body) = &step.call.body {
        carried.extend(substitutions(body).map(|name| ("body".to_owned(), name)));
    }
    for (header, value) in step.call.headers.iter().flatten() {
        carried.extend(substitutions(value).map(|name| (format!("headers.{header}"), name)));
    }

    for (where_it_is, name) in carried {
        if !captured.contains(name) {
            found.push(Violation {
                location: format!("{at}.call.{where_it_is}"),
                message: format!(
                    "substitutes {name}, which no earlier step captures, so what this call would \
                     carry cannot be read off the manifest"
                ),
            });
            continue;
        }
        let declared = recipe
            .pairs
            .iter()
            .any(|pair| pair.value == name && pair.to == step.call.to);
        if !declared {
            found.push(Violation {
                location: format!("{at}.call.{where_it_is}"),
                message: format!(
                    "carries {name} to {}, and no [[recipe.pair]] declares that; every value a \
                     flow could carry to every destination it could reach is declared, so that \
                     what this plugin tells whom is answerable without running it",
                    step.call.to
                ),
            });
        }
    }
}

/// Every name substituted into one piece of text.
fn substitutions(text: &str) -> impl Iterator<Item = &str> {
    text.split(OPENS)
        .skip(1)
        .filter_map(|after| after.split_once(CLOSES))
        .map(|(name, _)| name.trim())
}

#[cfg(test)]
mod tests;
