use crate::refusing::tests::{names, said, without, INSTALLABLE};

/// The check in the shared fixture, as its declaration is added to it.
const CHECK: &str = r#"fixture   = "fixtures/api-v1-claim-claimed.json"
timeout_s = 10"#;

/// The proof in the shared fixture, as its declaration is added to it.
const PROOF: &str = r#"fixture = "fixtures/health.json"
why     = "The path the health probe asks for is one this image serves.""#;

/// The check, declaring that it fails on one recording with this entry.
fn check_declaring(entries: &str) -> Vec<String> {
    without(CHECK, &format!("{CHECK}\nexpected  = [{entries}]"))
}

/// The proof, declaring that it fails on one recording with this entry.
fn proof_declaring(entries: &str) -> Vec<String> {
    without(PROOF, &format!("{PROOF}\nexpected = [{entries}]"))
}

/// A declaration naming a constraint its assertion makes, at a place it looks, with a
/// reason, is refused nothing — on a check and on a proof.
#[test]
fn a_declaration_that_describes_its_assertion_is_refused_nothing() {
    assert_eq!(
        check_declaring(
            r#"{ fixture = "fixtures/api-v1-claim-unclaimed.json", verdict = "fails", constraint = "json", place = "isClaimed", reason = "Recorded before anybody claimed it." }"#
        ),
        Vec::<String>::new()
    );
    assert_eq!(
        proof_declaring(
            r#"{ fixture = "fixtures/health.json", verdict = "fails", constraint = "status", reason = "Recorded while it was starting." }"#
        ),
        Vec::<String>::new()
    );
}

/// A declaration naming a verdict other than failing is refused by the shape, naming
/// the field and the one word it may hold.
#[test]
fn a_declaration_of_any_verdict_but_failing_is_refused() {
    for verdict in ["passes", "unproven"] {
        let refused = check_declaring(&format!(
            r#"{{ fixture = "f.json", verdict = "{verdict}", constraint = "status", reason = "r" }}"#
        ));
        assert!(
            names(&refused, &["verdict", "fails"]),
            "{verdict}: {refused:?}"
        );
    }
}

/// Each field a declaration needs is refused by name when it is absent.
#[test]
fn a_declaration_missing_a_field_is_refused_naming_it() {
    for (entry, field) in [
        (
            r#"{ verdict = "fails", constraint = "status", reason = "r" }"#,
            "fixture",
        ),
        (
            r#"{ fixture = "f.json", constraint = "status", reason = "r" }"#,
            "verdict",
        ),
        (
            r#"{ fixture = "f.json", verdict = "fails", reason = "r" }"#,
            "constraint",
        ),
        (
            r#"{ fixture = "f.json", verdict = "fails", constraint = "status" }"#,
            "reason",
        ),
    ] {
        let refused = check_declaring(entry);
        assert!(names(&refused, &[field]), "{field}: {refused:?}");
    }
}

/// A blank reason and a blank recording are refused as surely as absent ones.
#[test]
fn a_blank_reason_or_recording_is_refused() {
    let refused = check_declaring(
        r#"{ fixture = " ", verdict = "fails", constraint = "status", reason = " " }"#,
    );
    assert!(
        names(&refused, &["komga:claimed.expected", "names no recording"]),
        "{refused:?}"
    );
    assert!(
        names(&refused, &["expected  .reason", "is blank"]),
        "{refused:?}"
    );
}

/// A constraint the assertion does not make is refused, with the ones it does.
#[test]
fn a_constraint_the_assertion_does_not_make_is_refused_with_those_it_makes() {
    let refused = check_declaring(
        r#"{ fixture = "f.json", verdict = "fails", constraint = "content_type", reason = "r" }"#,
    );
    assert!(
        names(
            &refused,
            &[
                "contribution komga:claimed.expected f.json.constraint",
                "content_type is not a constraint",
                "it makes json, status"
            ]
        ),
        "{refused:?}"
    );
}

/// A key-wise constraint needs its place, and the place has to be one it looks at.
#[test]
fn a_key_wise_constraint_is_declared_at_a_place_it_looks() {
    let absent = check_declaring(
        r#"{ fixture = "f.json", verdict = "fails", constraint = "json", reason = "r" }"#,
    );
    assert!(
        names(&absent, &["f.json.place", "is absent", "isClaimed"]),
        "{absent:?}"
    );

    let elsewhere = check_declaring(
        r#"{ fixture = "f.json", verdict = "fails", constraint = "json", place = "/elsewhere", reason = "r" }"#,
    );
    assert!(
        names(
            &elsewhere,
            &["/elsewhere is not a place json constrains", "isClaimed"]
        ),
        "{elsewhere:?}"
    );
}

/// A constraint about the whole answer has no place within it.
#[test]
fn a_constraint_about_the_whole_answer_names_no_place() {
    let refused = check_declaring(
        r#"{ fixture = "f.json", verdict = "fails", constraint = "status", place = "isClaimed", reason = "r" }"#,
    );
    assert!(
        names(&refused, &["f.json.place", "about the answer as a whole"]),
        "{refused:?}"
    );
}

/// One recording is declared once.
#[test]
fn a_recording_named_twice_is_refused() {
    let refused = proof_declaring(
        r#"{ fixture = "f.json", verdict = "fails", constraint = "status", reason = "r" },
    { fixture = "f.json", verdict = "fails", constraint = "json", place = "status", reason = "r" }"#,
    );
    assert!(
        names(
            &refused,
            &[
                "proof komga.serves.expected f.json",
                "more than one declaration"
            ]
        ),
        "{refused:?}"
    );
}

/// A remedy asserts nothing, so it has nothing to declare, and a probe gates what
/// other services are wired on, so it may not excuse a failure.
#[test]
fn neither_a_remedy_nor_a_probe_may_declare_a_failure() {
    let entry = r#"expected = [{ fixture = "f.json", verdict = "fails", constraint = "status", reason = "r" }]"#;
    let remedy = without(
        "detail = \"POST /api/v1/claim",
        &format!("{entry}\ndetail = \"POST /api/v1/claim"),
    );
    assert!(
        names(&remedy, &["komga:claim-it", "expected"]),
        "{remedy:?}"
    );

    let probe = said(&INSTALLABLE.replacen(
        "fixture = \"fixtures/",
        &format!("{entry}\nfixture = \"fixtures/"),
        1,
    ));
    assert!(names(&probe, &["expected"]), "{probe:?}");
}

/// What a declaration says is read by whoever decides whether to trust the plugin, so
/// it is held to the same rule as every other string a manifest declares.
#[test]
fn a_reason_a_diff_cannot_show_is_refused() {
    let refused = check_declaring(
        r#"{ fixture = "f.json", verdict = "fails", constraint = "status", reason = "hidden\u0007" }"#,
    );
    assert!(names(&refused, &["f.json.reason", "U+0007"]), "{refused:?}");
}
