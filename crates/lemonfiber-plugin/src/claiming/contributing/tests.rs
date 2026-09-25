use super::contributed;
use crate::{Manifest, Violation};

/// The identities the doctor's register holds in these tests.
const OCCUPIED: &[&str] = &["storage.space", "environment.engine"];

/// A plugin declaring one check and the remedy that check carries.
///
/// The shape every case below varies one thing in, so what a case is about is the
/// line it changed rather than the fifty it repeated.
const DECLARING: &str = r#"
schema_version = 1

[plugin]
id          = "komga"
name        = "Komga"
version     = "1.1.0"
description = "Reads comics in a browser"
without_it  = "Comics stay folders of images"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "komga"
name        = "Komga"
image       = "example.invalid/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.26.3"
port        = 25600
bind        = "lan"
criticality = "enhancing"

[requires]
capabilities = ["doctor.contribute"]

[[contribution]]
at        = "doctor.check"
id        = "komga:claimed"
title     = "Komga has an administrator"
category  = "credentials"
request   = { method = "GET", path = "/api/v1/claim" }
expect    = { status = 200, json = { isClaimed = true } }
fixture   = "fixtures/claim.json"
why       = "An unclaimed Komga hands administrator to whoever asks first."

[[contribution]]
at     = "doctor.remedy"
id     = "komga:claim-it"
for    = "komga:claimed"
action = "Open Komga and create the administrator account"
why    = "Until somebody does, the first caller on the household network becomes it."
"#;

/// What the rules say about a manifest, as one sentence per violation.
fn against(text: &str) -> Vec<String> {
    // A fixture this build cannot read is a mistake in the test rather than a
    // refusal, and it has to stop the case rather than come back as an empty list
    // that every "says nothing" assertion below would pass on.
    let read = Manifest::from_toml(text);
    assert!(read.is_ok(), "the fixture does not read: {read:?}");
    let mut found: Vec<Violation> = Vec::new();
    if let Ok(manifest) = read {
        contributed(&manifest, OCCUPIED, &mut found);
    }
    found.iter().map(ToString::to_string).collect()
}

/// Whether some one sentence carries all of these.
fn says(found: &[String], words: &[&str]) -> bool {
    found
        .iter()
        .any(|said| words.iter().all(|word| said.contains(word)))
}

/// The fixture itself is refused for nothing, so every case below means something.
///
/// Without this, a rule that refused every manifest would pass every case that
/// asserts on what was refused, and the cases asserting silence would be the only
/// ones telling the truth.
#[test]
fn a_plugin_declaring_a_check_and_its_remedy_is_refused_for_nothing() {
    assert_eq!(against(DECLARING), Vec::<String>::new());
}

/// A remedy that would call a service is refused as a remedy, and told where to go.
///
/// The mistake is a real thing to want — a repair that puts the fault right rather
/// than describing it — so the refusal names the block that carries one instead of
/// answering that the field is not on the list.
#[test]
fn a_remedy_that_would_act_on_the_operator_s_system_is_refused_as_a_remedy() {
    let acting = DECLARING.replace(
        r#"action = "Open Komga and create the administrator account""#,
        "action = \"Claim it\"\nrequest = { method = \"POST\", path = \"/api/v1/claim\" }",
    );
    let said = against(&acting);
    assert!(
        says(&said, &["komga:claim-it", "request", "[[recipe]]"]),
        "{said:?}"
    );
    assert!(
        says(&said, &["rendered", "doctor.remedy"]) || says(&said, &["renders", "doctor.remedy"]),
        "the refusal says which point renders rather than runs: {said:?}"
    );
}

/// And the ordinary out-of-set refusal still happens, for a field that only says.
///
/// The half that keeps the new sentence from swallowing the old one. A row carrying
/// a field that belongs to the other point is usually a row written for the wrong
/// point, and telling its author about recipes would send them somewhere unhelpful.
#[test]
fn a_field_that_says_rather_than_does_is_refused_without_mentioning_recipes() {
    let misplaced = DECLARING.replace(
        r#"action = "Open Komga and create the administrator account""#,
        "action = \"Open Komga\"\ncategory = \"credentials\"",
    );
    let said = against(&misplaced);
    assert!(
        says(&said, &["category", "outside what", "doctor.remedy"]),
        "{said:?}"
    );
    assert!(
        !says(&said, &["[[recipe]]"]),
        "a field that only says is not a repair that acts: {said:?}"
    );
}

/// A check still requires the very field a remedy is refused for carrying.
///
/// The direction a rule keyed on the field name alone would get wrong: `request` is
/// how a check asks, and a refusal that fired wherever it appeared would make the
/// check undeclarable.
#[test]
fn the_field_a_remedy_may_not_carry_is_the_one_a_check_must() {
    let asking_nothing = DECLARING.replace(
        r#"request   = { method = "GET", path = "/api/v1/claim" }"#,
        "",
    );
    let said = against(&asking_nothing);
    assert!(
        says(&said, &["carries no request", "doctor.check"]),
        "{said:?}"
    );
    assert!(!says(&said, &["[[recipe]]"]), "{said:?}");
}
