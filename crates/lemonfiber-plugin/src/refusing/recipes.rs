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
const METHODS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE"];

/// Everything this build refuses about the recipes a manifest declares.
pub(super) fn declared(manifest: &Manifest, found: &mut Vec<Violation>) {
    if manifest.recipes.is_empty() {
        return;
    }
    asked_for(manifest, found);

    let mut named: BTreeSet<&str> = BTreeSet::new();
    for recipe in &manifest.recipes {
        let at = format!("recipe {}", recipe.id);
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
        calling(step, &here, found);
        substituting(step, recipe, &captured, &here, found);

        for capture in &step.capture {
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
    if looks_like_an_address(&step.call.to) {
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
mod tests {
    use super::{looks_like_an_address, substitutions};
    use crate::refusing::refusals;
    use crate::schema::tests::WHOLE;
    use crate::schema::Manifest;

    /// The identities lemonfiber's own registers hold, which the whole fixture avoids.
    const OCCUPIED: &[&str] = &["storage.hardlinks"];

    fn said(text: &str) -> Vec<String> {
        Manifest::from_toml(text).map_or_else(
            |refused| vec![refused.to_string()],
            |manifest| {
                refusals(&manifest, OCCUPIED)
                    .iter()
                    .map(ToString::to_string)
                    .collect()
            },
        )
    }

    fn names(said: &[String], words: &[&str]) -> bool {
        said.iter()
            .any(|one| words.iter().all(|word| one.contains(word)))
    }

    fn without(before: &str, after: &str) -> Vec<String> {
        assert!(WHOLE.contains(before), "the fixture still says {before:?}");
        said(&WHOLE.replace(before, after))
    }

    /// The declared block is read rather than skipped, and the capability is named.
    #[test]
    fn a_recipe_declared_without_the_capability_that_runs_it_is_refused_by_naming_it() {
        let said = without(r#""recipe.run""#, r#""doctor.contribute""#);
        assert!(
            names(&said, &["requires.capabilities", "recipe.run"]),
            "got: {said:?}"
        );
        assert!(
            !said.iter().any(|one| one.contains("version")),
            "and never by naming a version: {said:?}"
        );
    }

    /// A manifest with no recipe is not asked for the capability that runs one.
    #[test]
    fn a_manifest_declaring_no_recipe_is_not_asked_for_it() {
        let text = WHOLE
            .split("[[recipe]]")
            .next()
            .unwrap_or_default()
            .replace(r#", "recipe.run""#, "");
        let said = said(&format!(
            "{text}\n[requires]\ncapabilities = [\"doctor.contribute\"]\n"
        ));
        assert!(
            !names(&said, &["recipe.run"]),
            "nothing asks for it: {said:?}"
        );
    }

    #[test]
    fn a_value_carried_to_a_destination_no_pair_declares_is_refused() {
        let said = without(
            "[[recipe.pair]]\nvalue = \"token\"\nto    = \"komga\"",
            "[[recipe.pair]]\nvalue = \"token\"\nto    = \"elsewhere\"",
        );
        assert!(
            names(
                &said,
                &["recipe adopt-existing-library.step create", "token"]
            ),
            "got: {said:?}"
        );
    }

    #[test]
    fn a_substitution_no_earlier_step_captures_is_refused() {
        let said = without("{{token}}", "{{nothing}}");
        assert!(names(&said, &["nothing"]), "got: {said:?}");
    }

    #[test]
    fn a_call_to_an_address_rather_than_a_name_is_refused() {
        let said = without(
            r#"to = "komga", path = "/api/v1/libraries""#,
            r#"to = "10.0.0.5", path = "/api/v1/libraries""#,
        );
        assert!(names(&said, &["call.to", "10.0.0.5"]), "got: {said:?}");
    }

    #[test]
    fn a_method_outside_the_verbs_a_call_may_use_is_refused() {
        let said = without(
            r#"call    = { method = "POST", to = "komga", path = "/api/v1/libraries""#,
            r#"call    = { method = "TRACE", to = "komga", path = "/api/v1/libraries""#,
        );
        assert!(names(&said, &["call.method", "TRACE"]), "got: {said:?}");
    }

    #[test]
    fn an_address_is_told_from_a_name_by_its_last_label() {
        assert!(looks_like_an_address("10.0.0.5"));
        assert!(looks_like_an_address("komga:25600"));
        assert!(!looks_like_an_address("komga"));
        assert!(!looks_like_an_address("api.example.com"));
    }

    #[test]
    fn a_substitution_is_read_out_of_the_text_it_sits_in() {
        let found: Vec<&str> = substitutions("Bearer {{token}} for {{ library }}").collect();
        assert_eq!(found, vec!["token", "library"]);
        assert_eq!(substitutions("nothing here").count(), 0);
    }
}
