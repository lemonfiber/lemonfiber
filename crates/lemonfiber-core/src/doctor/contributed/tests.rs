use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_plugin::Manifest;

use super::{declared, CONTRIBUTED_FAILED};
use crate::doctor::{examine, Category, Check, Finding, Narrowing, Overall, Verdict};
use crate::error::{Problem, Remedy, State};
use crate::plugin::Installed;
use crate::ports::http::{Http, Method};

/// A plugin that declares one check on its own service and one remedy for it.
///
/// The whole manifest rather than a hand-built row, because what is being tested is
/// a plugin's declaration reaching the register — and a row assembled in Rust would
/// skip the reading, which is half of what a contribution is.
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
detail = "http://komga.local:25600"
why    = "Until somebody does, the first caller on the household network becomes it."
"#;

/// Where the plugin's one service answers.
fn answering() -> BTreeMap<String, String> {
    BTreeMap::from([("komga".to_owned(), "http://komga:25600".to_owned())])
}

/// The plugin's rows, against a transport the case may want to question afterwards.
///
/// A fixture this build cannot read is a mistake in the test rather than a thing
/// under test, and it has to stop the case rather than come back as an empty list
/// that every assertion about what was not found would pass on.
fn checks_of(text: &str, http: &Arc<dyn Http>) -> Vec<Box<dyn Check>> {
    let read = Manifest::from_toml(text);
    assert!(read.is_ok(), "the fixture does not read: {read:?}");
    read.as_ref()
        .map(|manifest| declared(&Installed::of(manifest), &answering(), http))
        .unwrap_or_default()
}

/// A transport that answers everything the same way.
fn saying(answer: Answer) -> Arc<dyn Http> {
    Fake::always(answer) as Arc<dyn Http>
}

/// The findings a plugin's rows produce against a transport that says this.
async fn found(text: &str, answer: Answer) -> Vec<Finding> {
    let checks = checks_of(text, &saying(answer));
    examine(&checks, &Narrowing::Suite).await.findings
}

/// The one finding a plugin's one row produces.
///
/// The stand-in below is never the answer to anything a case asserts, so a run that
/// somehow got past the count above still fails rather than quietly matching.
async fn only(text: &str, answer: Answer) -> Finding {
    let mut findings = found(text, answer).await;
    assert_eq!(findings.len(), 1, "one row, one finding: {findings:?}");
    // Built before it is needed rather than in a closure. A fallback reached
    // only when something has already gone wrong is a function no passing run
    // enters, and one nothing enters is a line the coverage gate is right to
    // count. It is never the answer to anything a case asserts, so a run that
    // somehow got past the count above still fails rather than quietly matching.
    let nothing = Finding::in_category(
        Category::Config,
        "no row was declared",
        "no row was declared",
        Verdict::Skipped {
            reason: "the fixture produced no finding at all".to_owned(),
        },
    );
    findings.pop().unwrap_or(nothing)
}

/// The problem a finding failed with, where it failed.
fn failed(finding: &Finding) -> Option<&Problem> {
    match &finding.verdict {
        Verdict::Fail(problem) => Some(problem),
        Verdict::Pass { .. }
        | Verdict::Warn(_)
        | Verdict::Unverified { .. }
        | Verdict::Skipped { .. } => None,
    }
}

/// Why a finding could not be established, and what to do to get an answer.
fn unrun(finding: &Finding) -> Option<(&str, &Remedy)> {
    match &finding.verdict {
        Verdict::Unverified { reason, remedy } => Some((reason.as_str(), remedy)),
        Verdict::Pass { .. } | Verdict::Warn(_) | Verdict::Fail(_) | Verdict::Skipped { .. } => {
            None
        }
    }
}

/// What a failing finding says to do, in the order it offers it.
fn actions(finding: &Finding) -> Vec<&str> {
    failed(finding)
        .map(|problem| {
            problem
                .remedies
                .iter()
                .map(|one| one.action.as_str())
                .collect()
        })
        .unwrap_or_default()
}

/// The answer a service gives when the declaration holds.
fn holding() -> Answer {
    Answer::served(200, "application/json", r#"{"isClaimed":true}"#)
}

/// The answer a service gives when it does not.
fn refuting() -> Answer {
    Answer::served(200, "application/json", r#"{"isClaimed":false}"#)
}

/// A row's whole finding is written in the vocabulary the doctor already has.
///
/// Nothing new reads a contribution, which is the property that keeps contributed
/// code from having anything to be: what comes out is the same category, the same
/// verdict type and the same error model a bundled check produces.
#[tokio::test]
async fn a_contributed_row_produces_a_finding_in_the_doctor_s_own_vocabulary() {
    let finding = only(DECLARING, holding()).await;
    assert_eq!(finding.check, "komga:claimed");
    assert_eq!(finding.category, Category::Credentials);
    assert_eq!(finding.title, "Komga has an administrator");
    assert_eq!(finding.service.as_deref(), Some("komga"));
    assert_eq!(
        finding.verdict,
        Verdict::Pass {
            note: Some("komga:claimed, contributed by komga answered as it declares".to_owned())
        }
    );
    // A row that holds fails at nothing and is not unrun, which is the half of
    // the vocabulary the cases below only ever see from the other side.
    assert!(failed(&finding).is_none(), "{finding:?}");
    assert!(unrun(&finding).is_none(), "{finding:?}");
}

/// A row that names which service it asks is about the one it names.
///
/// The other answer `asked_of` can give, and the one a plugin with more than one
/// service would always take. Without a case for it the only shape ever exercised
/// is the shorthand a single-service plugin gets for free, and the field a
/// manifest actually writes goes unread.
#[tokio::test]
async fn a_row_that_names_its_service_is_about_the_one_it_names() {
    let naming = DECLARING.replace(
        r#"category  = "credentials""#,
        "category  = \"credentials\"\nservice   = \"komga\"",
    );
    let finding = only(&naming, holding()).await;
    assert_eq!(finding.check, "komga:claimed");
    assert_eq!(finding.service.as_deref(), Some("komga"));
}

/// The evaluator is the one a plugin's recordings are judged by, live.
///
/// Asserted through the constraint that needs the answer's headers, because that is
/// the one a live run could quietly stop being able to decide: a transport that did
/// not carry back what it was served as would read every answer as served as
/// nothing, and every content type as a mismatch.
#[tokio::test]
async fn what_the_service_was_served_as_is_judged_rather_than_assumed() {
    let asking = DECLARING.replace(
        "expect    = { status = 200, json = { isClaimed = true } }",
        r#"expect    = { status = 200, content_type = "application/json" }"#,
    );
    let held = only(&asking, holding()).await;
    assert_eq!(
        held.verdict,
        Verdict::Pass {
            note: Some("komga:claimed, contributed by komga answered as it declares".to_owned())
        }
    );

    let xml = only(&asking, Answer::served(200, "text/xml", "<claim/>")).await;
    assert!(
        failed(&xml).is_some_and(|problem| problem
            .detail
            .as_ref()
            .is_some_and(|said| said.contains("text/xml"))),
        "a document served as the wrong thing is a fault that says so: {xml:?}"
    );
}

/// A declaration the answer refutes fails, carrying the remedy the plugin declared.
///
/// Guided rather than actionable or remediable, which is the whole of what a plugin
/// may contribute to remediation: the explanation and the next action, for the
/// operator to carry out.
#[tokio::test]
async fn a_refuted_row_fails_and_carries_the_remedy_the_plugin_declared() {
    let finding = only(DECLARING, refuting()).await;
    let problem = failed(&finding);
    assert_eq!(problem.map(|one| one.code), Some(CONTRIBUTED_FAILED));
    assert_eq!(problem.map(|one| one.state), Some(State::Guided));
    assert!(
        problem.is_some_and(|one| one.summary.contains("komga:claimed")
            && one.summary.contains("contributed by komga")),
        "{finding:?}"
    );
    assert_eq!(
        problem.map(|one| one.meaning.as_str()),
        Some("Until somebody does, the first caller on the household network becomes it.")
    );
    assert_eq!(
        problem.map(|one| one.remedies.clone()),
        Some(vec![Remedy::new(
            "Open Komga and create the administrator account"
        )
        .with_detail("http://komga.local:25600")])
    );
}

/// A remedy is text and stays text, however much its action reads like a command.
///
/// The half that would not fail on its own: a remedy carried as a string and never
/// run looks exactly like one that is run, from the outside, unless somebody counts
/// what the transport was asked. One request, and it is the check's own.
#[tokio::test]
async fn a_remedy_that_reads_like_a_command_is_rendered_and_never_run() {
    let shaped = DECLARING.replace(
        r#"action = "Open Komga and create the administrator account""#,
        r#"action = "curl -XPOST http://komga:25600/api/v1/claim""#,
    );
    let transport = Fake::always(refuting());
    let checks = checks_of(&shaped, &(Arc::clone(&transport) as Arc<dyn Http>));
    let report = examine(&checks, &Narrowing::Suite).await;

    let asked = transport.requests();
    assert_eq!(asked.len(), 1, "only the check itself asks: {asked:?}");
    assert_eq!(
        asked.first().map(|one| one.url.as_str()),
        Some("http://komga:25600/api/v1/claim")
    );
    assert_eq!(
        report.findings.first().map(actions),
        Some(vec!["curl -XPOST http://komga:25600/api/v1/claim"]),
        "the action is rendered exactly as it was written: {report:?}"
    );
}

/// Several remedies are offered in the order the plugin put them in.
///
/// Which is why a remedy is a row rather than a field: where more than one cause is
/// plausible they are listed by likelihood rather than asserted as certain, and the
/// leading one is what the finding is said to mean.
#[tokio::test]
async fn several_declared_remedies_are_offered_in_the_order_they_were_declared() {
    let two = format!(
        "{DECLARING}\n[[contribution]]\nat = \"doctor.remedy\"\nid = \"komga:restart\"\n\
             for = \"komga:claimed\"\naction = \"Restart Komga and try again\"\n\
             why = \"A half-started Komga answers before it has read its database.\"\n"
    );
    let finding = only(&two, refuting()).await;
    assert_eq!(
        actions(&finding),
        vec![
            "Open Komga and create the administrator account",
            "Restart Komga and try again"
        ]
    );
    assert!(
        failed(&finding).is_some_and(|one| one.meaning.starts_with("Until somebody does")),
        "{finding:?}"
    );
}

/// A failing check whose remedies could not be rendered still carries one.
///
/// A finding with nothing to do about it is a dead end with a plugin's name on it,
/// and the rule requiring a remedy has no exemption for a contributed finding.
#[tokio::test]
async fn a_failing_row_whose_remedy_cannot_be_rendered_still_carries_one() {
    let (check, _) = DECLARING
        .split_once("[[contribution]]\nat     = \"doctor.remedy\"")
        .unwrap_or_default();
    let finding = only(check, refuting()).await;
    let offered = actions(&finding);
    assert_eq!(offered.len(), 1, "{finding:?}");
    assert!(
        offered.iter().all(|one| one.contains("komga")),
        "the remedy names whose check this is: {offered:?}"
    );
}

/// A row whose service is not answering anywhere is unrun, not absent and not passed.
#[tokio::test]
async fn a_row_whose_service_cannot_be_reached_is_reported_unrun() {
    let read = Manifest::from_toml(DECLARING);
    assert!(read.is_ok(), "the fixture does not read: {read:?}");
    let checks = read
        .as_ref()
        .map(|manifest| {
            declared(
                &Installed::of(manifest),
                &BTreeMap::new(),
                &saying(holding()),
            )
        })
        .unwrap_or_default();
    let report = examine(&checks, &Narrowing::Suite).await;
    assert_eq!(report.findings.len(), 1, "{report:?}");
    assert_eq!(report.overall, Overall::Unknown);
    let said = report.findings.first().and_then(unrun).map(|(why, _)| why);
    assert!(
        said.is_some_and(|why| why.contains("komga:claimed") && why.contains("komga")),
        "{report:?}"
    );
}

/// A service that is there and does not answer is unrun for the reason it gave.
#[tokio::test]
async fn a_service_that_does_not_answer_is_unrun_for_the_reason_it_gave() {
    let finding = only(DECLARING, Answer::Silent).await;
    let said = unrun(&finding);
    assert!(
        said.is_some_and(|(why, _)| why.contains("connection refused")),
        "{finding:?}"
    );
    assert!(
        said.is_some_and(|(_, remedy)| remedy.action.contains("komga")),
        "{finding:?}"
    );
}

/// Every way a row can be unrunnable is reported as a row, never as an absence.
///
/// Each of these is refused when the manifest is read. That is exactly why they are
/// here: the register's promise is that a declared row produces a finding whatever
/// the row says, and a row that got past validation must not vanish instead.
#[tokio::test]
async fn a_row_that_cannot_be_turned_into_a_question_is_still_a_row() {
    let cases = [
        (
            "asks nothing",
            DECLARING.replace(
                r#"request   = { method = "GET", path = "/api/v1/claim" }"#,
                "",
            ),
        ),
        (
            "says nothing about the answer",
            DECLARING.replace(
                "expect    = { status = 200, json = { isClaimed = true } }",
                "",
            ),
        ),
        (
            "names a method nothing can send",
            DECLARING.replace(r#"method = "GET""#, r#"method = "TRACE""#),
        ),
        (
            "names a path that is not a route on its service",
            DECLARING.replace(r#"path = "/api/v1/claim""#, r#"path = "@elsewhere:9/x""#),
        ),
        (
            "names a service the plugin does not declare",
            DECLARING.replace(
                r#"fixture   = "fixtures/claim.json""#,
                "fixture   = \"fixtures/claim.json\"\nservice   = \"elsewhere\"",
            ),
        ),
    ];
    for (about, text) in cases {
        let finding = only(&text, holding()).await;
        assert!(unrun(&finding).is_some(), "{about}: {finding:?}");
        assert_eq!(finding.check, "komga:claimed", "{about}");
    }
}

/// A plugin with two services and a row naming neither is unrun, not guessed at.
///
/// The default only exists where there is one thing it could mean. A row checked
/// against whichever service the reader reached first would be a finding about a
/// container nobody asked about, reported under a name that says otherwise.
#[tokio::test]
async fn a_row_that_does_not_say_which_of_two_services_it_asks_is_unrun() {
    let two = DECLARING.replace(
            "[requires]",
            "[[service]]\nid          = \"komga-tools\"\nname        = \"Komga tools\"\n\
             image       = \"example.invalid/komga-tools\"\n\
             digest      = \"sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945\"\n\
             tag         = \"1.0.0\"\nport        = 25601\nbind        = \"loopback\"\n\
             criticality = \"enhancing\"\n\n[requires]",
        );
    let finding = only(&two, holding()).await;
    assert!(
        unrun(&finding).is_some_and(|(why, _)| why.contains("which of this plugin's services")),
        "{finding:?}"
    );
    assert_eq!(finding.service, None);
}

/// Every method the transport carries is one a row may name, however it spelled it.
///
/// The half a single case would miss: a row is refused for naming a method that
/// cannot be sent, and a rule recognising only the one the fixture happens to use
/// would refuse three that can be. Asserted on what the transport was handed,
/// because a row that reached it as `GET` whatever it declared would pass a test
/// that only read the verdict.
#[tokio::test]
async fn every_method_the_transport_carries_is_one_a_row_may_name() {
    for (declared, sent) in [
        (r#"method = "GET""#, Method::Get),
        (r#"method = "post""#, Method::Post),
        (r#"method = "Put""#, Method::Put),
        (r#"method = "DELETE""#, Method::Delete),
    ] {
        let transport = Fake::always(holding());
        let asking = DECLARING.replace(r#"method = "GET""#, declared);
        let checks = checks_of(&asking, &(Arc::clone(&transport) as Arc<dyn Http>));
        let report = examine(&checks, &Narrowing::Suite).await;
        assert_eq!(
            transport.requests().first().map(|one| one.method),
            Some(sent),
            "{declared} reached the transport as something else"
        );
        assert!(
            report
                .findings
                .first()
                .is_some_and(|one| matches!(one.verdict, Verdict::Pass { .. })),
            "{declared}: {report:?}"
        );
    }
}

/// A row naming a family the doctor does not have is reported rather than filed.
///
/// Filing it somewhere plausible and running it would put a verdict under a heading
/// its author did not choose; leaving it out would be the one outcome that must not
/// happen. So it is reported where manifest validity belongs, as not having run.
#[tokio::test]
async fn a_row_naming_a_family_the_doctor_does_not_have_is_reported_under_config() {
    let strange = DECLARING.replace(r#"category  = "credentials""#, r#"category  = "comics""#);
    let finding = only(&strange, holding()).await;
    assert_eq!(finding.category, Category::Config);
    assert_eq!(finding.check, "komga:claimed");
    assert!(
        unrun(&finding).is_some_and(|(why, _)| why.contains("comics")),
        "{finding:?}"
    );
}

/// A row naming no family at all is the same answer, and does not say "nothing".
///
/// Separate from the case above because the two reach it differently: one named a
/// family that is not one, and this one named none. A message reading "nothing is
/// not a family the doctor has" is the shape of a rule that had not thought about
/// the second.
#[tokio::test]
async fn a_row_naming_no_family_is_reported_rather_than_filed_anywhere() {
    let silent = DECLARING.replace("category  = \"credentials\"\n", "");
    let finding = only(&silent, holding()).await;
    assert_eq!(finding.category, Category::Config);
    assert!(
        unrun(&finding).is_some_and(|(why, _)| why.contains("komga:claimed")),
        "{finding:?}"
    );
}

/// The bounds are the published point's, not the plugin's opinion of them.
#[test]
fn a_row_is_bounded_by_what_the_point_publishes_rather_than_by_what_it_asked_for() {
    let budget = |asked: &str| {
        let text = DECLARING.replace(
            r#"fixture   = "fixtures/claim.json""#,
            &format!("fixture   = \"fixtures/claim.json\"\n{asked}"),
        );
        checks_of(&text, &saying(holding()))
            .first()
            .map(|check| check.budget())
    };
    assert_eq!(budget(""), Some(Duration::from_secs(10)));
    assert_eq!(budget("timeout_s = 3"), Some(Duration::from_secs(3)));
    assert_eq!(budget("timeout_s = 0"), Some(Duration::from_secs(1)));
    assert_eq!(budget("timeout_s = 600"), Some(Duration::from_secs(30)));
}

/// A contributed row runs in the very list a bundled check runs in.
#[tokio::test]
async fn a_contributed_row_runs_beside_the_bundled_checks_in_one_list() {
    let mut checks = vec![Box::new(Bundled) as Box<dyn Check>];
    checks.extend(checks_of(DECLARING, &saying(holding())));
    let report = examine(&checks, &Narrowing::Suite).await;
    let named: Vec<&str> = report
        .findings
        .iter()
        .map(|one| one.check.as_str())
        .collect();
    assert_eq!(named, vec!["storage.space", "komga:claimed"]);
    assert_eq!(report.overall, Overall::Healthy);
}

/// And a run narrowed to the plugin's own name runs that row and nothing else.
#[tokio::test]
async fn a_run_narrowed_to_a_contributed_name_runs_that_row_alone() {
    let mut checks = vec![Box::new(Bundled) as Box<dyn Check>];
    checks.extend(checks_of(DECLARING, &saying(holding())));
    let narrowing = Narrowing::parse("komga:claimed");
    assert_eq!(
        narrowing,
        Some(Narrowing::Check("komga:claimed".to_owned())),
        "the name an operator reads is the name they can ask for"
    );
    let report = examine(&checks, &narrowing.unwrap_or(Narrowing::Suite)).await;
    let named: Vec<&str> = report
        .findings
        .iter()
        .map(|one| one.check.as_str())
        .collect();
    assert_eq!(named, vec!["komga:claimed"]);
}

/// And a run narrowed to a bundled family leaves a plugin's row alone.
///
/// The other direction of the question above, and the one that asks each check
/// which family it belongs to. A contributed row filed under another family must
/// not answer to a bundled family's name, or narrowing would be a way of reaching
/// a plugin's check without naming it.
#[tokio::test]
async fn a_run_narrowed_to_a_bundled_family_leaves_a_plugin_s_row_alone() {
    let mut checks = vec![Box::new(Bundled) as Box<dyn Check>];
    checks.extend(checks_of(DECLARING, &saying(holding())));
    let report = examine(&checks, &Narrowing::Category(Category::Storage)).await;
    let named: Vec<&str> = report
        .findings
        .iter()
        .map(|one| one.check.as_str())
        .collect();
    assert_eq!(named, vec!["storage.space"]);
}

/// A row abandoned at its budget still says which row was abandoned.
///
/// A timeout reported against the family alone would leave nothing on the page to
/// attribute — and a contributed check that could not run has to be readable as
/// that plugin's check having not run, rather than as the family being unsure.
#[tokio::test(start_paused = true)]
async fn a_row_abandoned_at_its_budget_names_the_check_and_the_plugin() {
    let never = Arc::new(Hanging) as Arc<dyn Http>;
    let checks = checks_of(DECLARING, &never);
    let report = examine(&checks, &Narrowing::Suite).await;
    assert_eq!(report.findings.len(), 1, "{report:?}");
    let finding = report.findings.first();
    assert_eq!(finding.map(|one| one.check.as_str()), Some("komga:claimed"));
    assert_eq!(
        finding.and_then(|one| one.service.as_deref()),
        Some("komga")
    );
    assert!(
        finding.is_some_and(|one| unrun(one).is_some()),
        "{report:?}"
    );
}

/// A stack with the plugin removed answers exactly as one that never had it.
///
/// Equality of the whole report rather than an empty list of contributed rows: what
/// the rule promises is that the answer is the same, and a report can differ from
/// another in ways no assertion about absence would notice.
#[tokio::test]
async fn a_run_after_the_plugin_is_removed_is_the_run_of_a_build_that_never_saw_it() {
    let bundled = || vec![Box::new(Bundled) as Box<dyn Check>];

    let mut installed = bundled();
    installed.extend(checks_of(DECLARING, &saying(holding())));
    let with = examine(&installed, &Narrowing::Suite).await;

    // Withdrawal is the rows leaving the list they joined, so it is done that way
    // here rather than by building a second list without them: two lists written
    // the same way agree because they were written the same way, which is not the
    // question. The register holds a row against the plugin that declared it, and
    // that is what makes the removal expressible at all.
    let mut left = bundled();
    left.extend(checks_of(DECLARING, &saying(holding())));
    left.retain(|check| {
        check
            .reports()
            .is_none_or(|one| !one.check.starts_with("komga:"))
    });
    let withdrawn = examine(&left, &Narrowing::Suite).await;
    let never = examine(&bundled(), &Narrowing::Suite).await;

    assert_ne!(with, never, "the plugin's row was there to be withdrawn");
    assert_eq!(withdrawn, never);
}

/// A bundled check, standing in for the register a plugin's row joins.
struct Bundled;

#[async_trait]
impl Check for Bundled {
    fn category(&self) -> Category {
        Category::Storage
    }
    async fn run(&self) -> Vec<Finding> {
        vec![Finding::in_category(
            Category::Storage,
            "storage.space",
            "Room to grow",
            Verdict::Pass { note: None },
        )]
    }
}

/// A transport that never answers, to prove the budget is what abandons a row.
struct Hanging;

#[async_trait]
impl Http for Hanging {
    async fn send(
        &self,
        _request: &crate::ports::http::Request,
    ) -> Result<crate::ports::http::Response, crate::ports::http::Unreachable> {
        std::future::pending().await
    }
}
