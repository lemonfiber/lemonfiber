//! The values inside an assertion, held to what a runner could do with them.
//!
//! An assertion is a request and an expectation, and both carry values the published
//! schema can say the shape of and not the meaning of: a media type is a string to a
//! schema, and so is a key that names nowhere. Neither is refusable by a `pattern`
//! either — the refusal an author needs says *which* half of a media type is empty, or
//! *where* in a pointer the escape went wrong, and a pattern can only say that
//! something did not match.
//!
//! What both rules have in common is what they refuse: an assertion that reads as one
//! and evaluates to nothing. A wildcard `accept` asks for nothing in particular; a key
//! naming no place is never looked at. Either would leave a proof quietly checking less
//! than it says — which is the rule against a tolerated unknown, pointed at a value
//! instead of at a field.

use crate::pointing;
use crate::schema::{Expect, Manifest, Request};
use crate::Violation;

/// What a request may ask for, where it asks for anything at all.
///
/// One media type, because a probe is recorded and a recording is of one answer: a list
/// with preferences is a negotiation whose outcome the manifest cannot state, and a
/// wildcard asks for nothing in particular, which is what a request with no `accept`
/// already says and says more plainly.
pub(super) fn asking(manifest: &Manifest, found: &mut Vec<Violation>) {
    for (at, request) in asked(manifest) {
        let Some(accept) = &request.accept else {
            continue;
        };
        if let Err(why) = negotiable(accept) {
            found.push(Violation {
                location: format!("{at}.request.accept"),
                message: why,
            });
        }
    }
}

/// Whether a value is one media type this build would put on the wire.
fn negotiable(accept: &str) -> Result<(), String> {
    if accept.contains('*') {
        return Err(format!(
            "{accept} asks for a wildcard, which asks for nothing in particular; a request that \
             names no type already says that, and a probe records one representation"
        ));
    }
    if accept.contains(',') {
        return Err(format!(
            "{accept} asks for more than one representation; which of them came back would then \
             be the service's choice, and a recording is of one answer"
        ));
    }
    let mut parts = accept.split(';');
    let whole = parts.next().unwrap_or_default().trim();
    let Some((kind, sub)) = whole.split_once('/') else {
        return Err(format!(
            "{accept} is not a media type; one is written `type/subtype`, optionally followed by \
             `; name=value`"
        ));
    };
    token(kind, accept)?;
    token(sub, accept)?;
    for parameter in parts {
        let Some((name, value)) = parameter.trim().split_once('=') else {
            return Err(format!(
                "{accept} carries a parameter that is not one; a parameter is written `name=value`"
            ));
        };
        token(name, accept)?;
        token(value, accept)?;
    }
    Ok(())
}

/// One half of a media type or of one of its parameters.
///
/// The characters are HTTP's own set for a token, less the `*` refused above. Written
/// out rather than as a range check because the set is not a range: it is the letters,
/// the digits and fourteen punctuation marks, and anything outside it has to be quoted
/// or escaped on the wire — which is a thing this build will not do on an author's
/// behalf, because a value that needed quoting to be sent is one a reviewer cannot read
/// as what arrives.
fn token(part: &str, whole: &str) -> Result<(), String> {
    const ALSO: &str = "!#$%&'+-.^_`|~";
    if part.is_empty() {
        return Err(format!(
            "{whole} leaves one half of a media type empty; one is written `type/subtype`"
        ));
    }
    if let Some(outside) = part
        .chars()
        .find(|letter| !letter.is_ascii_alphanumeric() && !ALSO.contains(*letter))
    {
        return Err(format!(
            "{whole} carries {outside:?}, which a media type may not; what it may carry is \
             letters, digits and {ALSO}"
        ));
    }
    Ok(())
}

/// Whether every place an expectation looks at is one a runner could reach.
///
/// A key that names no place is worse than one that names the wrong place: it is not
/// evaluated, so the assertion silently checks less than it says — and a manifest is an
/// instruction, so it is refused by name here rather than reported as a failed proof
/// later, which would say the service is broken when the key is.
pub(super) fn looking(manifest: &Manifest, found: &mut Vec<Violation>) {
    for (at, expect) in expected(manifest) {
        for (field, key) in places(expect) {
            if let Err(why) = pointing::steps(key) {
                found.push(Violation {
                    location: format!("{at}.expect.{field}"),
                    message: format!("{key} names no place in an answer: {why}"),
                });
            }
        }
    }
}

/// Every place one expectation looks at, with the constraint that looks there.
///
/// The four key-wise constraints and no others: `json_array_min`, `json_is_absent`,
/// `content_type` and `body_starts_with` are about the answer as a whole rather than
/// about somewhere in it.
fn places(expect: &Expect) -> Vec<(&'static str, &str)> {
    let exact = expect
        .json
        .iter()
        .flatten()
        .map(|(key, _)| ("json", key.as_str()));
    let present = expect
        .json_has_keys
        .iter()
        .flatten()
        .map(|key| ("json_has_keys", key.as_str()));
    let kinds = expect
        .json_types
        .iter()
        .flatten()
        .map(|(key, _)| ("json_types", key.as_str()));
    let least = expect
        .json_at_least
        .iter()
        .flatten()
        .map(|(key, _)| ("json_at_least", key.as_str()));
    exact.chain(present).chain(kinds).chain(least).collect()
}

/// Every request a manifest declares, with where it was declared.
///
/// A recipe's call is not one of these. It is a different shape with a different rule
/// over it — it runs with lemonfiber's own authority, carries headers on purpose, and is
/// answered for by the pair analysis — and folding the two together would be putting one
/// rule over two things that differ in exactly the way that matters.
fn asked(manifest: &Manifest) -> Vec<(String, &Request)> {
    let mut every = Vec::new();
    for claim in &manifest.claims {
        for binding in &claim.probes {
            every.push((
                format!("claim {}.probe {}", claim.capability, binding.id),
                &binding.request,
            ));
        }
    }
    for proof in &manifest.proofs {
        every.push((format!("proof {}", proof.id), &proof.request));
    }
    for entry in &manifest.contributions {
        if let Some(request) = &entry.request {
            every.push((format!("contribution {}", entry.id), request));
        }
    }
    every
}

/// Every expectation a manifest declares, with where it was declared.
///
/// A recipe step's is here where its call is not, and the asymmetry is the point: a step
/// judges an answer with the same vocabulary everything else does, so a key that names
/// no place is the same fault wherever it is written.
fn expected(manifest: &Manifest) -> Vec<(String, &Expect)> {
    let mut every = Vec::new();
    for claim in &manifest.claims {
        for binding in &claim.probes {
            every.push((
                format!("claim {}.probe {}", claim.capability, binding.id),
                &binding.expect,
            ));
        }
    }
    for proof in &manifest.proofs {
        every.push((format!("proof {}", proof.id), &proof.expect));
    }
    for entry in &manifest.contributions {
        if let Some(expect) = &entry.expect {
            every.push((format!("contribution {}", entry.id), expect));
        }
    }
    for recipe in &manifest.recipes {
        for step in &recipe.steps {
            if let Some(expect) = &step.expect {
                every.push((format!("recipe {}.step {}", recipe.id, step.id), expect));
            }
        }
    }
    every
}

#[cfg(test)]
mod tests {
    // The fixtures the whole of `refusing` is driven against: a manifest this build
    // would act on, the one edit each case makes to it, and the two ways of reading
    // what came back. Shared rather than copied, so a rule proved against a fixture
    // nobody else uses is a rule proved against nothing in particular.
    use crate::refusing::tests::{names, said, without, INSTALLABLE};

    /// The representations a request may ask for, each one a service actually sends.
    #[test]
    fn a_request_may_ask_for_one_media_type() {
        for accept in [
            "application/json",
            "application/vnd.api+json",
            "application/json; charset=utf-8",
            "text/xml",
            "application/opds+json;profile=opds-catalog",
        ] {
            let said = without(
                r#"request = { method = "GET", path = "/api/v1/series" }
expect  = { status = 401 }"#,
                &format!(
                    "request = {{ method = \"GET\", path = \"/api/v1/series\", \
                     accept = \"{accept}\" }}\nexpect  = {{ status = 401 }}"
                ),
            );
            assert!(said.is_empty(), "{accept}: {said:?}");
        }
    }

    /// And the ways one is not a media type, each refused naming the value.
    #[test]
    fn a_request_asking_for_anything_but_one_media_type_is_refused_by_name() {
        for (accept, words) in [
            ("application/json, text/xml", "more than one representation"),
            ("*/*", "wildcard"),
            ("application/*", "wildcard"),
            ("json", "is not a media type"),
            ("application/", "leaves one half"),
            ("/json", "leaves one half"),
            ("application/json; charset", "parameter that is not one"),
            ("application/json@1", "which a media type may not"),
            ("application/js on", "which a media type may not"),
            // A control character would not survive the wire, and is refused here by
            // the same rule rather than by the sweep that reads every declared string:
            // this one names the field the author wrote it in.
            ("application/json\\u007F", "which a media type may not"),
        ] {
            let said = without(
                r#"request = { method = "GET", path = "/api/v1/series" }
expect  = { status = 401 }"#,
                &format!(
                    "request = {{ method = \"GET\", path = \"/api/v1/series\", \
                     accept = \"{accept}\" }}\nexpect  = {{ status = 401 }}"
                ),
            );
            assert!(
                names(&said, &["accept", words]),
                "{accept} was not refused for {words}: {said:?}"
            );
        }
    }

    /// The places an expectation may look, including the one Plex needs.
    #[test]
    fn an_expectation_may_look_wherever_a_pointer_reaches() {
        for key in [
            "content",
            "/content",
            "/MediaContainer/size",
            "/MediaContainer/Setting/[id=PublishServerOnPlexOnlineKey]/value",
            "/a~1b",
        ] {
            let said = without(
                r#"json_has_keys = ["content"], json_types = { content = "list" }"#,
                &format!(r#"json_has_keys = ["{key}"], json_types = {{ content = "list" }}"#),
            );
            assert!(said.is_empty(), "{key}: {said:?}");
        }
    }

    /// A key naming no place is refused when the manifest is read, never evaluated.
    ///
    /// Each of the four key-wise constraints, because a rule that looked at one of them
    /// would leave the other three carrying keys nothing can reach.
    #[test]
    fn an_expectation_key_naming_no_place_is_refused_by_name() {
        for (before, after, field) in [
            (
                r#"json_has_keys = ["content"]"#,
                r#"json_has_keys = ["/Setting[id=x]"]"#,
                "json_has_keys",
            ),
            (
                r#"json_types = { content = "list" }"#,
                r#"json_types = { "/a~2b" = "list" }"#,
                "json_types",
            ),
            (
                r#"expect  = { status = 200, json = { status = "UP" } }"#,
                r#"expect  = { status = 200, json = { "/[=x]" = "UP" } }"#,
                "json",
            ),
            (
                r#"expect  = { status = 200, json = { status = "UP" } }"#,
                r#"expect  = { status = 200, json_at_least = { "/a~" = 1 } }"#,
                "json_at_least",
            ),
        ] {
            let said = without(before, after);
            assert!(
                names(&said, &[field, "names no place in an answer"]),
                "{after}: {said:?}"
            );
        }
    }

    /// A recipe step's expectation is held to it too, where a step's call is not.
    #[test]
    fn a_recipe_steps_expectation_is_held_to_the_same_places() {
        let text = format!(
            "{INSTALLABLE}\n[[recipe]]\nid = \"seed\"\ntitle = \"Seed\"\nwhy = \"Because\"\n\
             [[recipe.step]]\nid = \"one\"\n\
             call = {{ method = \"GET\", to = \"komga\", path = \"/x\" }}\n\
             expect = {{ status = 200, json_has_keys = [\"/a~\"] }}\n"
        );
        let said = said(&text);
        assert!(
            names(
                &said,
                &["recipe seed.step one", "names no place in an answer"]
            ),
            "got: {said:?}"
        );
    }
}
