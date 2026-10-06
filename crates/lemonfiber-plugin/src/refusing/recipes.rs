//! What a recipe is refused for, decided by reading it.
//!
//! Everything about a recipe that can be decided without running one is decided here,
//! before a call is made: where each call goes, where each value comes from, that every
//! guard looks back and every wait is bounded, and the set of flows it could produce.
//!
//! **Running one is asked for by name.** A manifest declaring a recipe names the
//! capability that runs one, so a build that does not offer it refuses the manifest
//! naming that capability. Parsing the block and skipping it would install a plugin
//! whose declared behaviour is wider than its actual one, which is the tolerated unknown
//! the whole format exists to refuse.
//!
//! **The flows are the substitutions.** Every `{{name}}` in a call's headers, its body
//! or a query value is one value reaching that call's destination, and each has to be
//! declared as a pair. A guard can skip a step and never add one, so the set is every
//! substitution in every step whatever the guards say. That is what makes "what does
//! this plugin tell whom" a question answerable from the file rather than from a run.

use std::collections::BTreeSet;

use crate::schema::{Manifest, Recipe, Step, RUN};
use crate::Violation;

// Guards that look back, retries within the published bounds, and captures that read
// a place.
mod bounds;
// Where a call goes, and where a value comes from.
mod whither;

/// What begins a path's query, after which a value may be substituted.
const QUERY: char = '?';

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
        if !named.insert(&recipe.id) {
            found.push(Violation {
                location: at.clone(),
                message: "is declared twice, so naming one of them names both".to_owned(),
            });
        }
        whither::inputs(manifest, recipe, &at, found);
        flows(manifest, recipe, &at, found);
        paired(manifest, recipe, &at, found);
        bounds::waiting(recipe, &at, found);
    }
}

/// Whether a destination a manifest names is a host outside the stack, which a value
/// carried there leaves the machine for.
#[must_use]
pub fn outside(manifest: &Manifest, to: &str) -> bool {
    whither::classified(manifest, to) == Some(whither::Whither::Outside)
}

/// Every pair one recipe declares: a value it has, to a destination that is one.
///
/// A pair nothing uses is allowed, because declaring more than is used is conservative
/// and the account says what was declared. A pair naming a value the recipe never has
/// is not: it would put an approval in front of the operator for something that cannot
/// happen, which is a sentence they would weigh for nothing.
fn paired(manifest: &Manifest, recipe: &Recipe, at: &str, found: &mut Vec<Violation>) {
    let had: BTreeSet<&str> = values(recipe).collect();
    for (number, pair) in recipe.pairs.iter().enumerate() {
        let here = format!("{at}.pair #{}", number + 1);
        worded(&pair.value, &format!("{here}.value"), found);
        if !had.contains(pair.value.as_str()) {
            found.push(Violation {
                location: format!("{here}.value"),
                message: format!(
                    "{} is no value this recipe captures or brings in, so the pair permits \
                     nothing that could happen",
                    pair.value
                ),
            });
        }
        destination(&pair.to, &format!("{here}.to"), found);
        whither::reachable(manifest, &pair.to, &format!("{here}.to"), found);
        whither::returned(recipe, &pair.value, &pair.to, &format!("{here}.to"), found);
    }
}

/// Every value one recipe has: what it brings in and what any of its steps captures.
fn values(recipe: &Recipe) -> impl Iterator<Item = &str> {
    recipe.inputs.iter().map(|input| input.name.as_str()).chain(
        recipe
            .steps
            .iter()
            .flat_map(|step| step.capture.iter())
            .map(|capture| capture.name.as_str()),
    )
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
fn flows(manifest: &Manifest, recipe: &Recipe, at: &str, found: &mut Vec<Violation>) {
    let mut before = bounds::Before {
        steps: BTreeSet::new(),
        values: BTreeSet::new(),
    };
    for input in &recipe.inputs {
        let here = format!("{at}.input {}", input.name);
        worded(&input.name, &format!("{here}.name"), found);
        if !before.values.insert(&input.name) {
            found.push(twice(&here, &input.name));
        }
    }

    for step in &recipe.steps {
        let here = format!("{at}.step {}", step.id);
        worded(&step.id, &format!("{here}.id"), found);
        calling(step, &here, found);
        whither::reachable(manifest, &step.call.to, &format!("{here}.call.to"), found);
        whither::captured(manifest, step, &here, found);
        bounds::guarded(step, &before, &here, found);
        bounds::retried(step, &before, &here, found);
        bounds::read(step, &here, found);
        substituting(step, recipe, &before.values, &here, found);

        if !before.steps.insert(&step.id) {
            found.push(Violation {
                location: here.clone(),
                message: "is declared twice, so a verdict against it names two calls".to_owned(),
            });
        }
        for capture in &step.capture {
            worded(&capture.name, &format!("{here}.capture.name"), found);
            if !before.values.insert(&capture.name) {
                found.push(twice(&format!("{here}.capture"), &capture.name));
            }
        }
    }
}

/// Said of a value one recipe has twice, from two captures or an input and a capture.
fn twice(at: &str, name: &str) -> Violation {
    Violation {
        location: at.to_owned(),
        message: format!(
            "{name} is captured or brought in twice, so a later call substituting it could mean \
             either"
        ),
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
    written_out(step, at, found);
    let mut carried: Vec<(String, &str)> = queried(&step.call.path)
        .map(|name| ("path".to_owned(), name))
        .collect();
    if let Some(body) = &step.call.body {
        carried.extend(substitutions(body).map(|name| ("body".to_owned(), name)));
    }
    for (header, value) in step.call.headers.iter().flatten() {
        carried.extend(substitutions(value).map(|name| (format!("headers.{header}"), name)));
    }

    for (where_it_is, name) in carried {
        let here = format!("{at}.call.{where_it_is}");
        whither::returned(recipe, name, &step.call.to, &here, found);
        if !captured.contains(name) {
            found.push(Violation {
                location: format!("{at}.call.{where_it_is}"),
                message: format!(
                    "substitutes {name}, which no earlier step captures and no input brings in, \
                     so what this call would carry cannot be read off the manifest"
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

/// Refuse a substitution where a call is written out: its destination, its path's
/// segments, its query's names and its headers' names.
///
/// What a call reaches is fixed by reading it. A destination worked out while running,
/// or a resource chosen by something that came back, would make the set of things a
/// recipe could do one that only running it reveals.
fn written_out(step: &Step, at: &str, found: &mut Vec<Violation>) {
    if substitutions(&step.call.to).next().is_some() {
        found.push(Violation {
            location: format!("{at}.call.to"),
            message: "substitutes a value into where the call goes, which is written out so that \
                      it is read off the manifest rather than worked out while running"
                .to_owned(),
        });
    }
    let (segments, query) = step
        .call
        .path
        .split_once(QUERY)
        .unwrap_or((step.call.path.as_str(), ""));
    let named = query.split('&').filter_map(|pair| {
        pair.split_once('=')
            .map_or(Some(pair), |(name, _)| Some(name))
    });
    if substitutions(segments).next().is_some() || named.flat_map(substitutions).next().is_some() {
        found.push(Violation {
            location: format!("{at}.call.path"),
            message: "substitutes a value into the path itself; only a query value may carry one, \
                      after `?` on the value side of a `name=value`"
                .to_owned(),
        });
    }
    for header in named_headers(step) {
        found.push(Violation {
            location: format!("{at}.call.headers"),
            message: format!(
                "substitutes a value into the name of the header {header:?}; a header's name is a \
                 fixed identifier of the protocol, so it is written out and only its value may \
                 carry one"
            ),
        });
    }
}

/// Whether any recipe of this manifest substitutes a value into a header's name, which
/// is refused with a code of its own.
#[must_use]
pub fn names_a_header_by_substitution(manifest: &Manifest) -> bool {
    manifest
        .recipes
        .iter()
        .flat_map(|recipe| &recipe.steps)
        .any(|step| named_headers(step).next().is_some())
}

/// Every header of one call whose name carries a substitution.
fn named_headers(step: &Step) -> impl Iterator<Item = &str> {
    step.call
        .headers
        .iter()
        .flatten()
        .map(|(header, _)| header.as_str())
        .filter(|header| substitutions(header).next().is_some())
}

/// Every name substituted into the values of a path's query.
fn queried(path: &str) -> impl Iterator<Item = &str> {
    path.split_once(QUERY)
        .map(|(_, query)| query)
        .into_iter()
        .flat_map(|query| query.split('&'))
        .filter_map(|pair| pair.split_once('=').map(|(_, value)| value))
        .flat_map(substitutions)
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
