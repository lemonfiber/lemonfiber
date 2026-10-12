use crate::refusing::tests::{names, said, without, INSTALLABLE};
use crate::schema::Expect;
use crate::vocabulary::Constraint;

const SERVES: &str = r#"[[proof]]
id      = "komga.serves""#;

/// The fixture with one more proof, a run, appended after the one it declares.
fn running(run: &str, expect: &str) -> Vec<String> {
    said(&INSTALLABLE.replace(
        SERVES,
        &format!(
            "[[proof]]\nid      = \"komga.runs\"\ntitle   = \"It starts\"\nrun     = {run}\n\
             expect  = {expect}\nwhy     = \"It starts as the service runs it.\"\n\n{SERVES}"
        ),
    ))
}

#[test]
fn a_proof_may_run_the_images_own_program() {
    let said = running(
        r#"{ args = ["--version"], timeout_s = 30 }"#,
        r#"{ exit = 0, stdout_starts_with = "v1." }"#,
    );
    assert!(said.is_empty(), "{said:?}");
    let alone = without(
        r#"request = { method = "GET", path = "/actuator/health" }
expect  = { status = 200, json = { status = "UP" } }"#,
        r#"run     = { args = ["--version"] }
expect  = { exit = 0, stdout_starts_with = "v1." }"#,
    );
    assert!(alone.is_empty(), "{alone:?}");
}

#[test]
fn a_proof_declaring_both_or_neither_is_refused_by_name() {
    let both = without(
        r#"request = { method = "GET", path = "/actuator/health" }"#,
        r#"request = { method = "GET", path = "/actuator/health" }
run     = { args = ["--version"] }"#,
    );
    assert!(
        names(&both, &["proof komga.serves", "both request and run"]),
        "{both:?}"
    );
    let neither = without(
        r#"request = { method = "GET", path = "/actuator/health" }
"#,
        "",
    );
    assert!(
        names(&neither, &["proof komga.serves", "neither request nor run"]),
        "{neither:?}"
    );
}

#[test]
fn a_run_holds_from_one_to_sixty_four_arguments() {
    let expect = r#"{ exit = 0, stdout_starts_with = "v" }"#;
    let sixty_four = format!("{{ args = [{}] }}", vec!["\"a\""; 64].join(", "));
    assert!(running(&sixty_four, expect).is_empty());
    let sixty_five = format!("{{ args = [{}] }}", vec!["\"a\""; 65].join(", "));
    let refused = running(&sixty_five, expect);
    assert!(
        names(&refused, &["proof komga.runs.run.args", "65 arguments"]),
        "{refused:?}"
    );
    let none = running("{ args = [] }", expect);
    assert!(
        names(&none, &["proof komga.runs.run.args", "0 arguments"]),
        "{none:?}"
    );
}

#[test]
fn an_argument_holds_at_most_4096_bytes_and_no_nul() {
    let expect = r#"{ exit = 0, stdout_starts_with = "v" }"#;
    let longest = format!("{{ args = [\"{}\"] }}", "a".repeat(4096));
    assert!(running(&longest, expect).is_empty());
    let longer = format!("{{ args = [\"ok\", \"{}\"] }}", "a".repeat(4097));
    let refused = running(&longer, expect);
    assert!(
        names(&refused, &["proof komga.runs.run.args[1]", "4097 bytes"]),
        "{refused:?}"
    );
    let nul = running(r#"{ args = ["a\u0000b"] }"#, expect);
    assert!(
        names(&nul, &["proof komga.runs.run.args[0]", "null character"]),
        "{nul:?}"
    );
}

#[test]
fn a_run_is_given_from_one_to_one_hundred_and_twenty_seconds() {
    let expect = r#"{ exit = 0, stdout_starts_with = "v" }"#;
    for seconds in [1, 120] {
        let said = running(
            &format!(r#"{{ args = ["-v"], timeout_s = {seconds} }}"#),
            expect,
        );
        assert!(said.is_empty(), "{seconds}: {said:?}");
    }
    for seconds in [0, 121] {
        let said = running(
            &format!(r#"{{ args = ["-v"], timeout_s = {seconds} }}"#),
            expect,
        );
        assert!(
            names(
                &said,
                &[
                    "proof komga.runs.run.timeout_s",
                    &format!("is {seconds} seconds")
                ]
            ),
            "{seconds}: {said:?}"
        );
    }
}

#[test]
fn a_runs_expect_takes_only_exit_and_its_output() {
    let said = running(
        r#"{ args = ["-v"] }"#,
        r#"{ exit = 0, status = 200, body_starts_with = "v" }"#,
    );
    for key in ["status", "body_starts_with"] {
        assert!(
            names(
                &said,
                &[&format!("proof komga.runs.expect.{key}"), "only exit"]
            ),
            "{key}: {said:?}"
        );
    }
    assert!(
        !said.iter().any(|one| one.contains("expect.exit")),
        "{said:?}"
    );
}

#[test]
fn a_requests_expect_takes_neither_of_a_runs_keys() {
    let proof = without(
        r#"expect  = { status = 200, json = { status = "UP" } }"#,
        r#"expect  = { status = 200, json = { status = "UP" }, exit = 0, stdout_starts_with = "v" }"#,
    );
    for key in ["exit", "stdout_starts_with"] {
        assert!(
            names(
                &proof,
                &[&format!("proof komga.serves.expect.{key}"), "about a run"]
            ),
            "{key}: {proof:?}"
        );
    }
    let probe = without(
        "expect  = { status = 401 }",
        "expect  = { status = 401, exit = 0 }",
    );
    assert!(
        names(
            &probe,
            &["claim media.serve.probe guarded.expect.exit", "about a run"]
        ),
        "{probe:?}"
    );
    let check = without(
        "expect    = { status = 200, json = { isClaimed = true } }",
        "expect    = { status = 200, json = { isClaimed = true }, stdout_starts_with = \"v\" }",
    );
    assert!(
        names(
            &check,
            &[
                "contribution komga:claimed.expect.stdout_starts_with",
                "about a run"
            ]
        ),
        "{check:?}"
    );
}

#[test]
fn a_probe_or_a_check_declaring_a_run_is_refused_naming_it() {
    for (before, at) in [
        ("expect  = { status = 401 }", "claim 0.probe guarded.run"),
        (
            "expect    = { status = 200, json = { isClaimed = true } }",
            ".run: this build declares no such field",
        ),
    ] {
        let said = without(before, &format!("{before}\nrun = {{ args = [\"-v\"] }}"));
        assert!(names(&said, &[at, "no such field"]), "{at}: {said:?}");
    }
}

#[test]
fn every_proof_saying_only_how_it_ended_is_refused_naming_each() {
    let status = without(
        r#"expect  = { status = 200, json = { status = "UP" } }"#,
        "expect  = { status = 200 }",
    );
    assert!(
        names(&status, &["proof komga.serves.expect", "only a status"]),
        "{status:?}"
    );
    let exit = without(
        r#"request = { method = "GET", path = "/actuator/health" }
expect  = { status = 200, json = { status = "UP" } }"#,
        r#"run     = { args = ["-v"] }
expect  = { exit = 0 }"#,
    );
    assert!(
        names(
            &exit,
            &["proof komga.serves.expect", "only a status or an exit"]
        ),
        "{exit:?}"
    );
    let empty = without(
        r#"request = { method = "GET", path = "/actuator/health" }
expect  = { status = 200, json = { status = "UP" } }"#,
        r#"run     = { args = ["-v"] }
expect  = { exit = 0, stdout_starts_with = "" }"#,
    );
    assert!(names(&empty, &["only a status"]), "{empty:?}");
}

#[test]
fn one_proof_saying_more_carries_the_others() {
    let said = running(r#"{ args = ["-v"] }"#, "{ exit = 0 }");
    assert!(said.is_empty(), "{said:?}");
}

#[test]
fn every_key_written_is_named_in_the_order_the_vocabulary_declares() {
    let every = Expect {
        status: Some(200),
        json: Some(std::collections::BTreeMap::new()),
        json_has_keys: Some(Vec::new()),
        json_types: Some(std::collections::BTreeMap::new()),
        json_at_least: Some(std::collections::BTreeMap::new()),
        json_array_min: Some(0),
        json_is_absent: Some(false),
        content_type: Some(String::new()),
        body_starts_with: Some(String::new()),
        exit: Some(0),
        stdout_starts_with: Some(String::new()),
    };
    assert_eq!(every.written(), Constraint::EVERY.to_vec());
    assert_eq!(Expect::default().written(), Vec::<Constraint>::new());
}
