use std::path::{Path, PathBuf};

use lemonfiber_plugin::Manifest;

use lemonfiber_plugin::vocabulary::Constraint;

use super::{both, checked, proved, Asserted, Assertion, FailingAsDeclared, Verdict};

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

/// A recording beside the others, of the same request, answering this status and body.
fn recorded(at: &Path, named: &str, status: u16, json: &str) {
    let _ = std::fs::write(
        at.join(named),
        format!(
            r#"{{"recorded_from": "{PINNED}", "note": "The same read.",
                    "request": {{"method": "GET", "path": "/api/series"}},
                    "response": {{"status": {status}, "json": {json}}}}}"#
        ),
    );
}

/// The check, declaring the failures these entries describe.
fn check_declaring(entries: &str) -> String {
    changed(&[(
        "fixture   = \"fixtures/guarded.json\"\n",
        &format!("fixture   = \"fixtures/guarded.json\"\nexpected  = [{entries}]\n"),
    )])
}

/// The check's declaration that its own recording fails on its status.
const FAILS_ON_ITS_STATUS: &str = r#"{ fixture = "fixtures/guarded.json", verdict = "fails", constraint = "status", reason = "Recorded from a Kavita nobody has claimed." }"#;

/// What the one check came to.
fn verdict_of(text: &str, at: &Path) -> Option<Verdict> {
    checks(text, at).first().map(|one| one.verdict.clone())
}

/// A check whose passing state cannot be recorded, failing where it declares it does
/// on the one constraint it names, is failing as declared: not passed, not failed, and
/// carrying what the recording held and why.
#[test]
fn a_check_failing_where_it_declares_is_failing_as_declared() {
    let at = source("as-declared", 200);
    assert_eq!(
        verdict_of(&check_declaring(FAILS_ON_ITS_STATUS), &at),
        Some(Verdict::FailingAsDeclared {
            declared: vec![FailingAsDeclared {
                fixture: "fixtures/guarded.json".to_owned(),
                constraint: Constraint::Status,
                place: None,
                held: "200".to_owned(),
                reason: "Recorded from a Kavita nobody has claimed.".to_owned(),
            }],
        })
    );
}

/// The verdict reads as the fourth outcome, apart from the other three, with the place
/// said only where the constraint has one.
#[test]
fn failing_as_declared_is_its_own_outcome_on_the_wire() {
    let declared = |place: Option<&str>| Verdict::FailingAsDeclared {
        declared: vec![FailingAsDeclared {
            fixture: "fixtures/identity-anonymous.json".to_owned(),
            constraint: Constraint::Json,
            place: place.map(str::to_owned),
            held: "false".to_owned(),
            reason: "Nobody has claimed it.".to_owned(),
        }],
    };
    assert_eq!(
        serde_json::to_value(declared(Some("/MediaContainer/claimed"))).ok(),
        Some(serde_json::json!({
            "outcome": "failing-as-declared",
            "declared": [{
                "fixture": "fixtures/identity-anonymous.json",
                "constraint": "json",
                "place": "/MediaContainer/claimed",
                "held": "false",
                "reason": "Nobody has claimed it.",
            }],
        }))
    );
    assert_eq!(
        serde_json::to_value(declared(None))
            .ok()
            .and_then(|said| said.pointer("/declared/0/place").cloned()),
        None
    );
}

/// The declared constraint holding on the recording makes the declaration stale, and
/// that fails, naming it.
#[test]
fn a_declared_recording_that_passes_makes_the_declaration_stale() {
    let at = source("stale", 401);
    assert_eq!(
        verdict_of(&check_declaring(FAILS_ON_ITS_STATUS), &at),
        Some(Verdict::Failed {
            faults: vec![
                "fixtures/guarded.json is declared to fail on status, and status holds there, \
                 so the declaration is stale; it goes in the change that made the recording pass"
                    .to_owned()
            ],
        })
    );
}

/// A failure the declaration does not describe is failed, naming the declared
/// constraint and every constraint that failed, whether or not the declared one did.
#[test]
fn a_failure_the_declaration_does_not_name_is_failed_naming_every_fault() {
    let at = source("another-failure", 401);
    let declaring_json = |status: u16| {
        recorded(
            &at,
            "fixtures/guarded.json",
            status,
            r#"{"claimed": false}"#,
        );
        verdict_of(
            &changed(&[
                ("expect    = { status = 401 }\n", "expect    = { status = 401, json = { claimed = true } }\n"),
                (
                    "fixture   = \"fixtures/guarded.json\"\n",
                    "fixture   = \"fixtures/guarded.json\"\nexpected  = [{ fixture = \"fixtures/guarded.json\", verdict = \"fails\", constraint = \"json\", place = \"claimed\", reason = \"Unclaimed.\" }]\n",
                ),
            ]),
            &at,
        )
    };
    assert_eq!(
        declaring_json(500),
        Some(Verdict::Failed {
            faults: vec![
                "fixtures/guarded.json is declared to fail on json at claimed, and fails on: \
                 answered 500 where it declares 401; claimed is false, and it declares true"
                    .to_owned()
            ],
        })
    );
    assert!(
        matches!(declaring_json(401), Some(Verdict::FailingAsDeclared { .. })),
        "the same declaration, with only the named constraint failing"
    );
}

/// A declaration is about the recording it names. The check's own recording is held to
/// its expectation as written, so it has to pass there and fail as declared on the other.
#[test]
fn a_declaration_changes_the_verdict_on_its_own_recording_and_no_other() {
    let at = source("each-state", 401);
    recorded(&at, "fixtures/open.json", 200, "null");
    let on_open = r#"{ fixture = "fixtures/open.json", verdict = "fails", constraint = "status", reason = "Unclaimed." }"#;
    assert!(
        matches!(
            verdict_of(&check_declaring(on_open), &at),
            Some(Verdict::FailingAsDeclared { declared }) if declared.len() == 1
        ),
        "held on its own recording and failed as declared on the other"
    );

    let _ = std::fs::write(at.join("fixtures/guarded.json"), "");
    let unreadable = verdict_of(&check_declaring(on_open), &at);
    assert!(
        matches!(&unreadable, Some(Verdict::Unproven { why }) if why.contains("guarded.json")),
        "its own recording unread is unproven, whatever the declared one came to: {unreadable:?}"
    );

    recorded(&at, "fixtures/guarded.json", 200, "null");
    assert!(
        matches!(
            verdict_of(&check_declaring(on_open), &at),
            Some(Verdict::Failed { faults }) if faults == ["answered 200 where it declares 401"]
        ),
        "failing on its own recording is failed: that recording is not the declared one"
    );
}

/// Every declared recording is reported, each with its own reason.
#[test]
fn two_declared_recordings_are_both_reported() {
    let at = source("two-declared", 200);
    recorded(&at, "fixtures/open.json", 200, "null");
    let both_declared = check_declaring(&format!(
        r#"{FAILS_ON_ITS_STATUS}, {{ fixture = "fixtures/open.json", verdict = "fails", constraint = "status", reason = "Also unclaimed." }}"#
    ));
    assert!(
        matches!(
            verdict_of(&both_declared, &at),
            Some(Verdict::FailingAsDeclared { declared })
                if declared.iter().map(|one| one.reason.as_str()).collect::<Vec<_>>()
                    == ["Recorded from a Kavita nobody has claimed.", "Also unclaimed."]
        ),
        "both declarations are carried"
    );
}

/// A declared recording that is not there is refused by name, and the assertion is
/// unproven: a declaration never stands in for a run.
#[test]
fn a_declared_recording_that_is_not_there_is_refused_and_unproven() {
    let at = source("declared-nowhere", 401);
    let declaring = changed(&[(
        "fixture = \"fixtures/guarded.json\"\nwhy",
        "expected = [{ fixture = \"fixtures/nowhere.json\", verdict = \"fails\", constraint = \"status\", reason = \"r\" }]\nwhy",
    )]);
    let (asserted, refusals) = proofs(&declaring, &at);
    assert!(
        matches!(
            asserted.first().map(|one| &one.verdict),
            Some(Verdict::Unproven { why }) if why.contains("nowhere.json")
        ),
        "got: {asserted:?}"
    );
    assert_eq!(
        refusals,
        vec![
            "proof guarded.expected fixtures/nowhere.json: names a recording the plugin's \
             source does not hold; a failure is declared on a recording somebody can read"
                .to_owned()
        ]
    );

    let blank = changed(&[(
        "fixture = \"fixtures/guarded.json\"\nwhy",
        "expected = [{ fixture = \"\", verdict = \"fails\", constraint = \"status\", reason = \"r\" }]\nwhy",
    )]);
    let (asserted, refusals) = proofs(&blank, &at);
    assert!(
        matches!(
            asserted.first().map(|one| &one.verdict),
            Some(Verdict::Unproven { why }) if why.contains("names no recording")
        ),
        "got: {asserted:?}"
    );
    assert!(
        refusals.is_empty(),
        "a blank recording is the manifest's refusal to make, not this run's: {refusals:?}"
    );
}

/// A proof naming no recording and declaring none has nothing to be run against, and
/// says so rather than reading as held.
#[test]
fn a_proof_naming_no_recording_is_unproven() {
    let at = source("no-recording", 401);
    let silent = changed(&[("fixture = \"fixtures/guarded.json\"\nwhy", "why")]);
    let (asserted, _) = proofs(&silent, &at);
    assert!(
        matches!(
            asserted.first().map(|one| &one.verdict),
            Some(Verdict::Unproven { why }) if why.contains("names no recording")
        ),
        "got: {asserted:?}"
    );
}

/// A proof may declare a failure too, and failing as declared does not stop it being
/// installed; its own recording failing still does.
#[test]
fn a_proof_failing_as_declared_is_reported_apart_and_does_not_stop_an_install() {
    let at = source("proof-declared", 200);
    let declaring = changed(&[(
        "fixture = \"fixtures/guarded.json\"\nwhy",
        "fixture = \"fixtures/guarded.json\"\nexpected = [{ fixture = \"fixtures/guarded.json\", verdict = \"fails\", constraint = \"status\", reason = \"r\" }]\nwhy",
    )]);
    let (asserted, refusals) = proofs(&declaring, &at);
    assert!(
        matches!(
            asserted.first().map(|one| &one.verdict),
            Some(Verdict::FailingAsDeclared { .. })
        ),
        "got: {asserted:?}"
    );
    assert!(refusals.is_empty(), "{refusals:?}");
    assert!(
        super::super::installs(&[], &[], &asserted),
        "failing as declared is not a failed proof"
    );
}

/// Two verdicts about one assertion come to one. A failure fails the run whatever else
/// was found, and names what could not be run beside it; what could not be run is
/// never read as shown; failing as declared is never read as passed.
#[test]
fn two_verdicts_about_one_assertion_come_to_one() {
    let failed = |fault: &str| Verdict::Failed {
        faults: vec![fault.to_owned()],
    };
    let unproven = |why: &str| Verdict::Unproven {
        why: why.to_owned(),
    };
    let entry = |reason: &str| FailingAsDeclared {
        fixture: "f.json".to_owned(),
        constraint: Constraint::Status,
        place: None,
        held: "200".to_owned(),
        reason: reason.to_owned(),
    };
    let declared = |reason: &str| Verdict::FailingAsDeclared {
        declared: vec![entry(reason)],
    };
    assert_eq!(
        both(failed("one"), failed("two")),
        Verdict::Failed {
            faults: vec!["one".to_owned(), "two".to_owned()],
        }
    );
    assert_eq!(
        both(unproven("unread"), failed("stale")),
        Verdict::Failed {
            faults: vec!["stale".to_owned(), "unread".to_owned()],
        }
    );
    assert_eq!(
        both(failed("stale"), unproven("unread")),
        Verdict::Failed {
            faults: vec!["stale".to_owned(), "unread".to_owned()],
        }
    );
    assert_eq!(both(Verdict::Passed, failed("one")), failed("one"));
    assert_eq!(both(declared("r"), failed("one")), failed("one"));
    assert_eq!(both(failed("one"), Verdict::Passed), failed("one"));
    assert_eq!(both(unproven("a"), unproven("b")), unproven("a; b"));
    assert_eq!(both(declared("r"), unproven("a")), unproven("a"));
    assert_eq!(both(unproven("a"), Verdict::Passed), unproven("a"));
    assert_eq!(
        both(declared("r"), declared("s")),
        Verdict::FailingAsDeclared {
            declared: vec![entry("r"), entry("s")],
        }
    );
    assert_eq!(both(Verdict::Passed, declared("r")), declared("r"));
    assert_eq!(both(declared("r"), Verdict::Passed), declared("r"));
    assert_eq!(both(Verdict::Passed, Verdict::Passed), Verdict::Passed);
}
