use std::path::{Path, PathBuf};

use super::{claimed, Shown, Unreadable, Verdict, BUNDLED_CHECKS};

/// A plugin's source, as one lands on a reviewer's disk.
const MANIFEST: &str = r#"
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
provides    = ["media.serve", "kavita:opds"]

[[claim]]
capability = "media.serve"

[[claim.probe]]
id      = "guarded"
request = { method = "GET", path = "/api/series" }
expect  = { status = 401 }
fixture = "fixtures/guarded.json"

[[claim.probe]]
id      = "catalogue"
request = { method = "GET", path = "/api/series" }
expect  = { status = 200, json_has_keys = ["content"] }
fixture = "fixtures/catalogue.json"
"#;

/// The image the manifest above pins, as a recording names it.
const PINNED: &str = "example.invalid/kavita@sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945";

fn guarded() -> String {
    recording(PINNED, "GET", "/api/series", r#"{"status": 401}"#)
}

fn catalogue() -> String {
    recording(
        PINNED,
        "GET",
        "/api/series",
        r#"{"status": 200, "json": {"content": []}}"#,
    )
}

fn recording(from: &str, method: &str, path: &str, response: &str) -> String {
    format!(
        r#"{{"recorded_from": "{from}", "note": "n",
                "request": {{"method": "{method}", "path": "{path}"}},
                "response": {response}}}"#
    )
}

/// A plugin source written to a scratch directory, with whatever recordings.
fn source(named: &str, manifest: &str, fixtures: &[(&str, String)]) -> PathBuf {
    let at = lemonfiber_fixtures::scratch::Scratch::named(&format!("claimed-{named}")).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(at.join("fixtures"));
    let _ = std::fs::write(at.join("plugin.toml"), manifest);
    for (name, body) in fixtures {
        let _ = std::fs::write(at.join(name), body);
    }
    at
}

/// A whole plugin, with both recordings answering what the claim declares.
fn whole(named: &str) -> PathBuf {
    source(
        named,
        MANIFEST,
        &[
            ("fixtures/guarded.json", guarded()),
            ("fixtures/catalogue.json", catalogue()),
        ],
    )
}

/// The state of the one core capability, and what its probes came to.
fn core(at: &Path) -> Option<(Shown, Vec<Verdict>)> {
    let read = claimed(at).ok()?;
    let claiming = read
        .capabilities
        .into_iter()
        .find(|claiming| !claiming.own)?;
    Some((
        claiming.shown,
        claiming.probes.into_iter().map(|ran| ran.verdict).collect(),
    ))
}

#[test]
fn a_claim_whose_recordings_answer_it_is_demonstrated() {
    let at = whole("demonstrated");
    assert_eq!(
        core(&at),
        Some((Shown::Demonstrated, vec![Verdict::Passed, Verdict::Passed]))
    );
    assert!(claimed(&at).is_ok_and(|read| read.installable));
}

/// The whole contract is what the probes ask together, so one refused probe refutes
/// the claim — and a refuted claim is a plugin that does not install.
#[test]
fn a_recording_that_refuses_a_probe_refutes_the_claim_and_stops_the_install() {
    let at = source(
        "refuted",
        MANIFEST,
        &[
            (
                "fixtures/guarded.json",
                recording(PINNED, "GET", "/api/series", r#"{"status": 200}"#),
            ),
            ("fixtures/catalogue.json", catalogue()),
        ],
    );
    let read = core(&at);
    assert_eq!(
        read.as_ref().map(|(shown, _)| shown.clone()),
        Some(Shown::Refuted)
    );
    assert!(
        read.as_ref().is_some_and(|(_, verdicts)| matches!(
            verdicts.first(),
            Some(Verdict::Failed { faults }) if !faults.is_empty()
        )),
        "got: {read:?}"
    );
    assert!(claimed(&at).is_ok_and(|read| !read.installable));
}

/// A recording that is not there establishes nothing, which is not the same as the
/// service being broken — so the claim is unproven and the plugin still installs.
#[test]
fn a_recording_that_is_absent_leaves_the_claim_unproven_rather_than_false() {
    let at = source("absent", MANIFEST, &[("fixtures/guarded.json", guarded())]);
    let read = core(&at);
    assert_eq!(
        read.as_ref().map(|(shown, _)| shown.clone()),
        Some(Shown::Unproven)
    );
    assert!(
        read.as_ref().is_some_and(|(_, verdicts)| matches!(
            verdicts.get(1),
            Some(Verdict::Unproven { why }) if why.contains("catalogue")
        )),
        "got: {read:?}"
    );
    assert!(claimed(&at).is_ok_and(|read| read.installable));
}

/// The two halves have drifted, and nothing about the service has been established
/// either way. Reporting it as a failure would say the service is broken when the
/// recording is.
#[test]
fn a_recording_of_another_call_is_unproven_and_names_both_calls() {
    let at = source(
        "drifted",
        MANIFEST,
        &[
            (
                "fixtures/guarded.json",
                recording(PINNED, "GET", "/api/v2/series", r#"{"status": 401}"#),
            ),
            ("fixtures/catalogue.json", catalogue()),
        ],
    );
    let read = core(&at);
    assert_eq!(
        read.as_ref().map(|(shown, _)| shown.clone()),
        Some(Shown::Unproven)
    );
    assert!(
        read.as_ref().is_some_and(|(_, verdicts)| matches!(
            verdicts.first(),
            Some(Verdict::Unproven { why })
                if why.contains("/api/v2/series") && why.contains("/api/series")
        )),
        "got: {read:?}"
    );
}

/// A recording of another build passes while describing software nobody is
/// installing, which is exactly what a pin moved without re-recording leaves.
#[test]
fn a_recording_from_another_image_is_refused_naming_both() {
    let elsewhere = recording(
            "example.invalid/kavita@sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "GET",
            "/api/series",
            r#"{"status": 401}"#,
        );
    let at = source(
        "elsewhere",
        MANIFEST,
        &[
            ("fixtures/guarded.json", elsewhere),
            ("fixtures/catalogue.json", catalogue()),
        ],
    );
    let read = claimed(&at).ok();
    let said: Vec<String> = read
        .as_ref()
        .map(|read| read.refusals.iter().map(ToString::to_string).collect())
        .unwrap_or_default();
    assert!(
        said.iter()
            .any(|one| one.contains("fixtures/guarded.json") && one.contains("0000000")),
        "got: {said:?}"
    );
    assert_eq!(
        read.as_ref().map(|read| read.installable),
        Some(false),
        "a refusal stops the install"
    );
    // Refused rather than run against: the recording answers what the binding
    // declares, and a verdict off it would be about the wrong image.
    assert_eq!(
        core(&at).map(|(shown, verdicts)| (shown, verdicts.first().cloned())),
        Some((
            Shown::Unproven,
            Some(Verdict::Unproven {
                why: said.first().cloned().unwrap_or_default()
            })
        ))
    );
}

/// The condition the plugin sets on its own install, as an author writes one.
const PROVING: &str = r#"
[[proof]]
id      = "reachable"
title   = "It answers on the path it says it is reached at"
request = { method = "GET", path = "/api/health" }
expect  = { status = 200 }
fixture = "fixtures/health.json"
why     = "A reader nothing can reach is one nobody can open."
"#;

/// A plugin carrying that condition, with whatever its recording answered.
fn proving(named: &str, status: u16) -> PathBuf {
    source(
        named,
        &format!("{MANIFEST}{PROVING}"),
        &[
            ("fixtures/guarded.json", guarded()),
            ("fixtures/catalogue.json", catalogue()),
            (
                "fixtures/health.json",
                recording(
                    PINNED,
                    "GET",
                    "/api/health",
                    &format!("{{\"status\": {status}}}"),
                ),
            ),
        ],
    )
}

/// A proof is the plugin's own condition for being installed, and a recording that
/// refuses one stops the install the way a refuted claim does.
///
/// Read through the whole report rather than through the runner, because what a
/// proof decides is the install — one rule over the proofs and the claims together,
/// which nothing asking about a single verdict can see. The claim here answers
/// either way, so the only thing moving is the proof.
#[test]
fn a_proof_its_recording_refuses_stops_the_install_and_one_it_answers_does_not() {
    let held = claimed(&proving("proved", 200)).ok();
    assert_eq!(
        held.as_ref().map(|read| (
            read.proofs.iter().map(|one| one.id.clone()).collect(),
            read.proofs.iter().map(|one| one.verdict.clone()).collect(),
            read.installable
        )),
        Some((vec!["reachable".to_owned()], vec![Verdict::Passed], true))
    );

    let refused = claimed(&proving("unproved", 503)).ok();
    assert_eq!(
        refused.as_ref().map(|read| read.installable),
        Some(false),
        "a proof its own recording refuses is a condition the plugin set and failed"
    );
    assert!(
        refused.is_some_and(|read| read.proofs.iter().any(|one| matches!(
            &one.verdict,
            Verdict::Failed { faults } if !faults.is_empty()
        ))),
        "and the verdict says what the answer was"
    );
}

/// The claim a bundled service also makes is contested, and every claimant is
/// named — which is what an operator resolves it by choosing from.
#[test]
fn a_core_capability_the_bundled_stack_also_claims_is_contested_and_names_everyone() {
    let at = whole("contested");
    let filling = claimed(&at).ok().and_then(|read| {
        read.capabilities
            .into_iter()
            .find(|claiming| !claiming.own)
            .and_then(|claiming| claiming.filling)
    });
    // Read off the rendering rather than destructured, because an arm for the
    // answers this is not would be a line no run enters.
    let said = format!("{filling:?}");
    assert!(said.contains("Contested"), "got: {said}");
    assert!(
        said.contains("jellyfin"),
        "the bundled claimants are named: {said}"
    );
    assert!(
        said.contains("kavita (plugin kavita)"),
        "and so is the plugin's own: {said}"
    );
}

/// Nothing asks for a namespaced capability, so there is no answer to give about
/// what fills it — which is a different thing from nothing filling it.
#[test]
fn a_capability_of_the_plugins_own_is_inert_rather_than_unfilled() {
    let at = whole("inert");
    let own = claimed(&at).ok().and_then(|read| {
        read.capabilities
            .into_iter()
            .find(|claiming| claiming.own)
            .map(|claiming| (claiming.name, claiming.filling, claiming.shown))
    });
    assert_eq!(own, Some(("kavita:opds".to_owned(), None, Shown::Claimed)));
}

#[test]
fn a_core_name_with_no_claim_is_unproven_as_well_as_refused() {
    let (before, _) = MANIFEST.split_once("[[claim]]").unwrap_or_default();
    let at = source("unclaimed", before, &[]);
    let read = claimed(&at).ok();
    assert_eq!(
        read.as_ref().and_then(|read| {
            read.capabilities
                .iter()
                .find(|claiming| !claiming.own)
                .map(|claiming| claiming.shown.clone())
        }),
        Some(Shown::Unproven)
    );
    assert_eq!(
        read.as_ref().map(|read| read.refusals.is_empty()),
        Some(false),
        "and the vocabulary says why"
    );
}

/// A core-looking name nothing publishes has no answer about what fills it, and
/// that is not the answer a namespaced one gets: one is a capability that does not
/// exist and the other is inert by design.
#[test]
fn a_core_name_the_vocabulary_does_not_carry_fills_nothing_and_is_not_inert() {
    let at = source(
        "unpublished",
        &MANIFEST.replace("media.serve", "media.stream"),
        &[("fixtures/guarded.json", guarded())],
    );
    let read = claimed(&at).ok();
    assert_eq!(
        read.as_ref().and_then(|read| {
            read.capabilities
                .iter()
                .find(|claiming| !claiming.own)
                .map(|claiming| (claiming.name.clone(), claiming.filling.clone()))
        }),
        Some(("media.stream".to_owned(), None))
    );
    assert_eq!(
        read.as_ref().map(|read| read.installable),
        Some(false),
        "and the manifest is refused"
    );
}

#[test]
fn a_path_with_no_manifest_says_that_rather_than_anything_about_a_plugin() {
    let at = lemonfiber_fixtures::scratch::Scratch::named("claimed-nothing-here");
    let _ = std::fs::remove_dir_all(&at);
    assert!(matches!(claimed(&at), Err(Unreadable::NoManifest(_))));
}

/// What a manifest contributes comes back in the terms a listing shows it.
///
/// Both halves of the line a row is shown by: a check says its title, a remedy has
/// none and says its action instead. Nothing else here reads an accepted
/// contribution at all, so the listing was only ever seen on a refused one.
#[test]
fn a_contributed_row_is_listed_by_what_it_says() {
    let contributing = format!(
        r#"{MANIFEST}
[requires]
capabilities = ["doctor.contribute"]

[[contribution]]
at        = "doctor.check"
id        = "kavita:claimed"
title     = "Kavita has an administrator"
category  = "credentials"
request   = {{ method = "GET", path = "/api/health" }}
expect    = {{ status = 200 }}
fixture   = "fixtures/guarded.json"
why       = "An unclaimed Kavita hands administrator to whoever asks first."

[[contribution]]
at     = "doctor.remedy"
for    = "kavita:claimed"
id     = "kavita:claim-it"
action = "Open Kavita and create the administrator account"
why    = "Until somebody does, the first caller on the household network becomes it."
"#
    );
    let at = source(
        "contributing",
        &contributing,
        &[
            ("fixtures/guarded.json", guarded()),
            ("fixtures/catalogue.json", catalogue()),
        ],
    );
    let read = claimed(&at).ok();
    let said: Vec<String> = read
        .as_ref()
        .map(|read| {
            read.contributions
                .iter()
                .map(|row| format!("{} {} {}", row.at, row.id, row.says))
                .collect()
        })
        .unwrap_or_default();
    // Built on a line that always runs: a message an assertion computes for itself
    // only runs where it fails, which is a line nothing covers.
    let refused: Vec<String> = read
        .map(|read| read.refusals.iter().map(ToString::to_string).collect())
        .unwrap_or_default();
    assert_eq!(
        said,
        vec![
            "doctor.check kavita:claimed Kavita has an administrator".to_owned(),
            "doctor.remedy kavita:claim-it Open Kavita and create the administrator \
                 account"
                .to_owned(),
        ],
        "refusals: {refused:?}"
    );
}

/// The register a contribution is held against is the doctor's own, not an empty
/// list handed in from here.
///
/// The rule that a contributed row may not take a bundled identity is only a rule
/// while the identities it is asked about are the real ones — and nothing else in
/// this crate would notice if this reader started passing none.
#[test]
fn a_contribution_is_held_against_the_identities_the_doctor_actually_holds() {
    let colliding = format!(
        r#"{MANIFEST}
[[contribution]]
at        = "doctor.check"
id        = "{}"
title     = "A row wearing a bundled name"
category  = "storage"
request   = {{ method = "GET", path = "/api/health" }}
expect    = {{ status = 200 }}
fixture   = "fixtures/guarded.json"
why       = "It should be refused for the name rather than for the row."
"#,
        BUNDLED_CHECKS.first().copied().unwrap_or_default()
    );
    let at = source(
        "occupied",
        &colliding,
        &[
            ("fixtures/guarded.json", guarded()),
            ("fixtures/catalogue.json", catalogue()),
        ],
    );
    let said: Vec<String> = claimed(&at)
        .ok()
        .map(|read| read.refusals.iter().map(ToString::to_string).collect())
        .unwrap_or_default();
    assert!(
        said.iter()
            .any(|one| one.contains("bundled row already holds")),
        "got: {said:?}"
    );
}

/// Each way a source can be unreadable says which one it was.
///
/// Four refusals with nothing to do with each other: a path holding no plugin, a
/// file that cannot be read, a manifest this build cannot read, and this build's
/// own pinned stack failing to publish. Whoever hit one needs a different answer to
/// each, which is the whole reason they are not one "invalid plugin".
#[test]
fn each_way_a_source_is_unreadable_says_which_one_it_was() {
    let said = |problem: Unreadable| problem.to_string();
    assert!(
        said(Unreadable::NoManifest(PathBuf::from("/somewhere"))).contains("/somewhere"),
        "the path it looked in is named"
    );
    assert!(
        said(Unreadable::Unopenable(std::io::Error::other("a disk"))).contains("a disk"),
        "and what the disk said"
    );
    assert!(
        said(Unreadable::Refused(
            lemonfiber_plugin::Error::UnsupportedSchema {
                found: 9,
                supported: vec![1],
            }
        ))
        .contains('9'),
        "and the generation a manifest declared"
    );
    assert!(
        said(Unreadable::Stack(super::Ungenerated::Unrenderable)).contains("JSON"),
        "and this build's own failure to publish"
    );
}

/// The file and the directory are both what somebody has to hand.
#[test]
fn the_manifest_itself_is_a_path_this_reads() {
    let at = whole("named-file");
    assert_eq!(
        claimed(&at.join("plugin.toml")).ok().map(|read| read.id),
        Some("kavita".to_owned())
    );
}
