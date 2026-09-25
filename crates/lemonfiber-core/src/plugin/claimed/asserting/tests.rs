use std::path::{Path, PathBuf};

use lemonfiber_plugin::Manifest;

use super::{checked, proved, Asserted, Assertion, Verdict};

/// A plugin whose evidence is a proof and a contributed check rather than a claim.
///
/// Written as an author writes it rather than built from the types. What these cases
/// are about is the two blocks nothing else in this crate declares — every manifest
/// under test until now carried `[[claim]]` and neither of these — so a fixture
/// assembled in code would be evidence about a shape no `plugin.toml` produces.
const ASSERTING: &str = r#"
schema_version = 1

[plugin]
id          = "kavita"
name        = "Kavita"
version     = "1.0.0"
description = "Reads comics in a browser"
without_it  = "Comics stay folders of images"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "kavita"
name        = "Kavita"
image       = "example.invalid/kavita"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.0.0"
port        = 5000
bind        = "lan"
criticality = "enhancing"

[[proof]]
id      = "guarded"
title   = "It refuses a read nobody signed in for"
request = { method = "GET", path = "/api/series" }
expect  = { status = 401 }
fixture = "fixtures/guarded.json"
why     = "A reader answering anybody is the household's library on the household's network."

[requires]
capabilities = ["doctor.contribute"]

[[contribution]]
at        = "doctor.check"
id        = "kavita:claimed"
title     = "Kavita has an administrator"
category  = "credentials"
request   = { method = "GET", path = "/api/series" }
expect    = { status = 401 }
fixture   = "fixtures/guarded.json"
why       = "An unclaimed Kavita hands administrator to whoever asks first."

[[contribution]]
at     = "doctor.remedy"
for    = "kavita:claimed"
id     = "kavita:claim-it"
action = "Open Kavita and create the administrator account"
why    = "Until somebody does, the first caller on the household network becomes it."
"#;

/// A second service, so a declaration naming none has two to choose between.
///
/// A plugin is often a thing and the thing beside it — a reader and the reader of
/// its history — which is what makes *which of these does this ask* a question an
/// author has to answer rather than a formality.
const ALONGSIDE: &str = r#"
[[service]]
id          = "kavita-sync"
name        = "Kavita's reading history"
image       = "example.invalid/kavita-sync"
digest      = "sha256:0000000000000000000000000000000000000000000000000000000000000000"
tag         = "1.0.0"
criticality = "enhancing"
"#;

/// The image the manifest above pins, as a recording names it.
const PINNED: &str = "example.invalid/kavita@sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945";

/// The fixture with some of its lines replaced, each guarded so a case cannot
/// quietly stop departing from the manifest the others are read against.
fn changed(edits: &[(&str, &str)]) -> String {
    let mut text = ASSERTING.to_owned();
    for (from, to) in edits {
        assert!(text.contains(from), "the fixture no longer says: {from}");
        text = text.replace(from, to);
    }
    text
}

/// A plugin source on disk, whose one recording answered with this status.
///
/// The recording moves and the manifest does not, because a refuted proof is a
/// service answering something other than what its author declared — and a case
/// that edited the declaration instead would be about a different manifest.
fn source(named: &str, answered: u16) -> PathBuf {
    let at = lemonfiber_fixtures::scratch::Scratch::named(&format!("asserting-{named}")).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(at.join("fixtures"));
    let _ = std::fs::write(
        at.join("fixtures/guarded.json"),
        format!(
            r#"{{"recorded_from": "{PINNED}", "note": "An unauthenticated read.",
                    "request": {{"method": "GET", "path": "/api/series"}},
                    "response": {{"status": {answered}}}}}"#
        ),
    );
    at
}

/// What a manifest's proofs come to, and whatever they refused on the way.
///
/// Read through the manifest reader rather than built, so a fixture that stopped
/// being a manifest stops the case here instead of coming back as a plugin
/// declaring nothing — against which an assertion about an empty list would pass.
fn proofs(text: &str, at: &Path) -> (Vec<Asserted>, Vec<String>) {
    let read = Manifest::from_toml(text);
    assert!(read.is_ok(), "the fixture does not read: {read:?}");
    let mut refusals = Vec::new();
    let asserted = read
        .map(|manifest| proved(&manifest, at, &mut refusals))
        .unwrap_or_default();
    (asserted, refusals.iter().map(ToString::to_string).collect())
}

/// The same, for the checks a manifest contributes.
fn checks(text: &str, at: &Path) -> Vec<Asserted> {
    let read = Manifest::from_toml(text);
    assert!(read.is_ok(), "the fixture does not read: {read:?}");
    let mut refusals = Vec::new();
    read.map(|manifest| checked(&manifest, at, &mut refusals))
        .unwrap_or_default()
}

/// A proof is the one assertion that gates an install and is not about a capability,
/// and it is reported against the recording it names like any other.
///
/// Every manifest this crate was read against declared `[[claim]]` and no
/// `[[proof]]`, so the block an author writes to say *this must hold before you
/// install me* was read by the manifest reader and never once evaluated.
#[test]
fn a_proof_is_reported_against_the_recording_it_names() {
    let at = source("proved", 401);
    let (asserted, refusals) = proofs(ASSERTING, &at);
    assert_eq!(
        asserted,
        vec![Asserted {
            kind: Assertion::Proof,
            id: "guarded".to_owned(),
            says: "It refuses a read nobody signed in for".to_owned(),
            service: "kavita".to_owned(),
            verdict: Verdict::Passed,
        }]
    );
    assert!(refusals.is_empty(), "got: {refusals:?}");
}

/// A proof its own recording refuses says how the answer fell short, which is the
/// half of the verdict an exit status cannot carry.
#[test]
fn a_proof_its_recording_refuses_says_how_the_answer_fell_short() {
    let at = source("refuted", 200);
    let (asserted, _) = proofs(ASSERTING, &at);
    assert_eq!(
        asserted.first().map(|one| one.verdict.clone()),
        Some(Verdict::Failed {
            faults: vec!["answered 200 where it declares 401".to_owned()],
        })
    );
}

/// A contributed check is run the same way a proof is, and the remedy beside it is
/// not run at all: it asks nothing and expects nothing, so running it would be
/// reporting a verdict about a sentence.
#[test]
fn a_contributed_check_is_run_and_the_remedy_beside_it_is_not() {
    let at = source("checked", 401);
    assert_eq!(
        checks(ASSERTING, &at),
        vec![Asserted {
            kind: Assertion::Check,
            id: "kavita:claimed".to_owned(),
            says: "Kavita has an administrator".to_owned(),
            service: "kavita".to_owned(),
            verdict: Verdict::Passed,
        }]
    );
}

/// A row that asks nothing establishes nothing, and unproven is the honest answer.
///
/// The rules refuse such a row, and the report is produced anyway — an author needs
/// to see the row that was refused rather than a plugin with nothing in it — so a
/// verdict has to be reached about it. Passed would read as a check that held.
#[test]
fn a_check_that_asks_nothing_is_unproven_rather_than_held() {
    let at = source("asks-nothing", 401);
    let silent = changed(&[
        (
            "request   = { method = \"GET\", path = \"/api/series\" }\n",
            "",
        ),
        ("expect    = { status = 401 }\n", ""),
    ]);
    let asserted = checks(&silent, &at);
    assert_eq!(asserted.first().map(|one| one.kind), Some(Assertion::Check));
    assert!(
        asserted.first().is_some_and(|one| matches!(
            &one.verdict,
            Verdict::Unproven { why } if why.contains("nothing to decide")
        )),
        "got: {asserted:?}"
    );
}

/// A declaration that does not settle which service it asks establishes nothing: the
/// service is what a recording's digest is held to, and there is no sensible guess.
///
/// Refused when the manifest is read, and reached here as well, because a verdict is
/// still reported for the row — and one that read as held would be saying a proof
/// passed against an image nobody has picked.
#[test]
fn a_proof_naming_no_service_where_a_plugin_declares_two_settles_nothing() {
    let at = source("unsettled", 401);
    let (asserted, _) = proofs(&format!("{ASSERTING}{ALONGSIDE}"), &at);
    assert_eq!(
        asserted.first().map(|one| one.service.clone()),
        Some(String::new()),
        "and it names none rather than the first one it reached"
    );
    assert!(
        asserted.first().is_some_and(|one| matches!(
            &one.verdict,
            Verdict::Unproven { why } if why.contains("does not settle which")
        )),
        "got: {asserted:?}"
    );
}

/// A proof naming one of two services is run against that one.
#[test]
fn a_proof_naming_which_service_it_asks_is_run_against_that_one() {
    let at = source("named", 401);
    let named = changed(&[(
        "why     = \"A reader",
        "service = \"kavita\"\nwhy     = \"A reader",
    )]);
    let (asserted, _) = proofs(&format!("{named}{ALONGSIDE}"), &at);
    assert_eq!(
        asserted
            .first()
            .map(|one| (one.service.clone(), one.verdict.clone())),
        Some(("kavita".to_owned(), Verdict::Passed))
    );
}

/// A proof naming a service this plugin does not declare settles nothing either.
///
/// The same answer as naming none, and a different mistake: one is an author who did
/// not say, the other is an author who said something their own manifest
/// contradicts, and neither leaves an image a recording could be held to.
#[test]
fn a_proof_naming_a_service_the_plugin_does_not_declare_settles_nothing() {
    let at = source("unknown-service", 401);
    let stranger = changed(&[(
        "why     = \"A reader",
        "service = \"somebody-elses\"\nwhy     = \"A reader",
    )]);
    let (asserted, _) = proofs(&stranger, &at);
    assert!(
        asserted.first().is_some_and(|one| one.service.is_empty()
            && matches!(
                &one.verdict,
                Verdict::Unproven { why } if why.contains("no image")
            )),
        "got: {asserted:?}"
    );
}
