//! The rows a plugin adds to the doctor's own register.
//!
//! A contributed check is not a second mechanism beside the doctor and not a hook into
//! it. It is another value in the same list the bundled checks are values in, run by
//! the same function, bounded the same way, reporting the same four verdicts, and
//! judged by the very evaluator a plugin's recordings are judged by. Nothing
//! contributed executes, because there is nothing for contributed code to *be*: what
//! arrives is a row, and everything that reads it is lemonfiber's.
//!
//! The row is turned into the engine's own vocabulary here and nowhere else. A family
//! becomes a category the doctor already has, a timeout becomes a budget within the
//! bounds the published point declares rather than the ones the plugin would like, and
//! a declared remedy becomes the error model's remedy — text, rendered, never run. A
//! contribution that needed something outside all of that would need an interpreter,
//! and there is nowhere to write one.
//!
//! **A row that cannot be run is reported, not dropped.** That is the rule the rest of
//! this module bends to: every declared check produces a finding on every run, and a
//! check nothing could ask becomes an unverified finding naming the check and the
//! plugin. A contribution that quietly disappeared would leave the stack looking
//! healthy for the wrong reason, which is worse than one that fails.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_plugin::extension;
use lemonfiber_plugin::{Contribution, Expect, Manifest, Request};

use super::{Category, Check, Finding, Reported, Verdict};
use crate::error::{Code, Problem, Remedy, Severity, State};
use crate::plugin::judging::judge;
use crate::plugin::recorded::Answer;
use crate::ports::http::{self, Http, Method};

/// A check a plugin contributed did not hold.
///
/// One code for all of them rather than one per plugin, because a code is a stable
/// thing an operator searches for and a plugin's own name is not this build's to mint
/// one from. Which check and which plugin is on the finding, where it can be read.
pub const CONTRIBUTED_FAILED: Code = Code::new("PLUGIN-1");

/// Every check an installed plugin adds to the register, ready to be run with the rest.
///
/// `answering` says where each of the plugin's services can be reached, by the id the
/// manifest gives it. A service missing from it is one lemonfiber cannot ask, which is
/// a reason to report a check unrun rather than a reason to leave it out.
///
/// Nothing is returned for a plugin that contributes nothing, and nothing at all for no
/// plugins — which is what makes a run after a plugin is removed the same run as one on
/// a build that never saw it, rather than one with a gap where it used to be.
#[must_use]
pub fn declared(
    manifest: &Manifest,
    answering: &BTreeMap<String, String>,
    http: &Arc<dyn Http>,
) -> Vec<Box<dyn Check>> {
    manifest
        .contributions
        .iter()
        .filter(|entry| entry.at == extension::check())
        .map(|entry| {
            Box::new(Contributed {
                plugin: manifest.plugin.id.clone(),
                check: entry.id.clone(),
                title: entry.title.clone().unwrap_or(entry.id.clone()),
                category: family(entry).unwrap_or(UNPLACEABLE),
                service: asked_of(manifest, entry),
                budget: bounded(entry.timeout_s),
                asks: asked(manifest, entry, answering),
                remedies: remedies(manifest, &entry.id),
                http: Arc::clone(http),
            }) as Box<dyn Check>
        })
        .collect()
}

/// The family a row is narrowed to, where it names one the doctor has.
///
/// Nothing where it does not, which is a row that cannot be run rather than one to be
/// filed somewhere plausible: a finding under a family its author did not choose is in
/// the wrong place on the page, and running the check anyway would put a verdict there.
/// It is refused when the manifest is read, so this is only the answer for a row that
/// reached here regardless — and leaving that one out of the register altogether is the
/// one outcome that must not happen.
fn family(entry: &Contribution) -> Option<Category> {
    entry.category.as_deref().and_then(Category::parse)
}

/// Where a row whose family the doctor does not have is filed.
///
/// Manifest validity is what `config` is for, and a row has to be filed somewhere to be
/// reported at all. What it reports there is that it could not be run.
const UNPLACEABLE: Category = Category::Config;

/// How long this check may run, within the bounds the published point sets.
///
/// The bounds are the ones the extension points publish, which is the same value the
/// point itself is built from — so a check is bounded by what lemonfiber says rather
/// than by the plugin's opinion of how long its service deserves. A row outside them is
/// refused when the manifest is read; one that reached here anyway is held to them
/// rather than believed.
fn bounded(asked: Option<u32>) -> Duration {
    let limits = extension::CHECK_TIMEOUT;
    let seconds = asked
        .unwrap_or(limits.default)
        .clamp(limits.min, limits.max);
    Duration::from_secs(u64::from(seconds))
}

/// Which of the plugin's services a row is about.
///
/// The manifest's own answer rather than a second one here: the rule that a
/// declaration may leave the service out where a plugin declares a single one is the
/// format's, and a copy of it beside each reader is a copy free to fall behind.
fn asked_of(manifest: &Manifest, entry: &Contribution) -> Option<String> {
    manifest
        .asks(entry.service.as_deref())
        .map(|service| service.id.clone())
}

/// What this check asks, or why nothing can be asked.
#[derive(Debug)]
enum Asks {
    /// Everything the engine needs to put the question.
    Service {
        /// Where the plugin's service answers.
        address: String,
        /// The method, as the transport carries it.
        method: Method,
        /// The path on that service.
        path: String,
        /// What the answer has to be.
        expect: Box<Expect>,
    },
    /// Why the question cannot be put, in the words the finding carries.
    Nothing(String),
}

/// The question this row puts, where every part of it is there.
fn asked(manifest: &Manifest, entry: &Contribution, answering: &BTreeMap<String, String>) -> Asks {
    if family(entry).is_none() {
        return Asks::Nothing(format!(
            "{} is not a family the doctor has, so there is nowhere to report it",
            entry.category.as_deref().unwrap_or("nothing")
        ));
    }
    let Some(service) = asked_of(manifest, entry) else {
        return Asks::Nothing(
            "it does not settle which of this plugin's services it asks".to_owned(),
        );
    };
    let Some(address) = answering.get(&service) else {
        return Asks::Nothing(format!(
            "{service} is not answering anywhere lemonfiber can reach"
        ));
    };
    let (Some(request), Some(expect)) = (&entry.request, &entry.expect) else {
        return Asks::Nothing(
            "it asks nothing, or says nothing about the answer, so there is nothing to establish"
                .to_owned(),
        );
    };
    let Some(method) = method(request) else {
        return Asks::Nothing(format!(
            "{} is not a method lemonfiber can send",
            request.method
        ));
    };
    Asks::Service {
        address: address.clone(),
        method,
        path: request.path.clone(),
        expect: Box::new(expect.clone()),
    }
}

/// The method a row names, as the transport carries it.
///
/// Four, because four is what the port has. A row naming anything else cannot be sent,
/// and saying so is a better answer than sending a different method than the one that
/// was declared.
fn method(request: &Request) -> Option<Method> {
    match request.method.to_ascii_uppercase().as_str() {
        "GET" => Some(Method::Get),
        "POST" => Some(Method::Post),
        "PUT" => Some(Method::Put),
        "DELETE" => Some(Method::Delete),
        _ => None,
    }
}

/// The remedies this same plugin declared for this check, in the order it declared them.
///
/// Text, in the shape the error model already has, and that is the whole of what a
/// contributed remedy is. A repair that acts is a recipe and is declared as one; there
/// is nothing here that could run.
fn remedies(manifest: &Manifest, check: &str) -> Vec<Guided> {
    manifest
        .contributions
        .iter()
        .filter(|entry| entry.at == extension::remedy())
        .filter(|entry| entry.about.as_deref() == Some(check))
        .filter_map(|entry| {
            entry.action.as_ref().map(|action| Guided {
                means: entry.why.clone().unwrap_or_default(),
                remedy: match &entry.detail {
                    Some(detail) => Remedy::new(action).with_detail(detail),
                    None => Remedy::new(action),
                },
            })
        })
        .collect()
}

/// One declared remedy, as what it means and what to do about it.
#[derive(Debug)]
struct Guided {
    /// What the finding means, so the action is not a ritual.
    means: String,
    /// What to do, as the error model carries it.
    remedy: Remedy,
}

/// A row a plugin added to the diagnostics register.
struct Contributed {
    /// The plugin that declared it, which every line about it names.
    plugin: String,
    /// What the finding is reported against, and what a run can be narrowed to.
    check: String,
    /// The one-line summary of what was checked.
    title: String,
    /// The family it is filed under.
    category: Category,
    /// The service the finding is about, where the row settles which.
    service: Option<String>,
    /// How long it may run, within the bounds the published point sets.
    budget: Duration,
    /// The question it puts, or why none can be put.
    asks: Asks,
    /// What the plugin says to do about it, in the order it said them.
    remedies: Vec<Guided>,
    /// The transport the question goes out over.
    http: Arc<dyn Http>,
}

#[async_trait]
impl Check for Contributed {
    fn category(&self) -> Category {
        self.category
    }

    fn budget(&self) -> Duration {
        self.budget
    }

    fn reports(&self) -> Option<Reported> {
        Some(Reported {
            check: self.check.clone(),
            service: self.service.clone(),
        })
    }

    async fn run(&self) -> Vec<Finding> {
        vec![self.found(self.verdict().await)]
    }
}

impl Contributed {
    /// The finding this row produces, attributed to the plugin that declared it.
    fn found(&self, verdict: Verdict) -> Finding {
        Finding {
            check: self.check.clone(),
            category: self.category,
            title: self.title.clone(),
            verdict,
            service: self.service.clone(),
            caused_by: None,
            said: None,
        }
    }

    /// What asking the service came to.
    ///
    /// One match over both shapes, so that neither is reached by having fallen past
    /// the other — a branch nothing can arrive at is a branch nobody can be sure of.
    async fn verdict(&self) -> Verdict {
        match &self.asks {
            Asks::Nothing(why) => self.unrun(why),
            Asks::Service {
                address,
                method,
                path,
                expect,
            } => self.asking(address, *method, path, expect).await,
        }
    }

    /// The question put, and what came back made into a verdict.
    async fn asking(&self, address: &str, method: Method, path: &str, expect: &Expect) -> Verdict {
        // No headers. A contributed check cannot present a credential, because there is
        // nowhere in the row to write one — which is what keeps "this asks as nobody"
        // a property of the format rather than a convention somebody has to hold.
        let answered = self
            .http
            .send(&http::Request {
                method,
                url: format!("{}{path}", address.trim_end_matches('/')),
                headers: Vec::new(),
                body: None,
            })
            .await;
        match answered {
            Err(unreachable) => self.unrun(&format!(
                "{} did not answer: {}",
                unreachable.url, unreachable.reason
            )),
            Ok(response) => self.judged(expect, &answer(&response)),
        }
    }

    /// The verdict the shared evaluator reached about what came back.
    fn judged(&self, expect: &Expect, answer: &Answer) -> Verdict {
        let faults = judge(expect, answer);
        if faults.is_empty() {
            return Verdict::Pass {
                note: Some(format!("{} answered as it declares", self.of())),
            };
        }
        Verdict::Fail(self.problem(&faults))
    }

    /// The problem a failing contributed check amounts to.
    ///
    /// Guided rather than actionable or remediable, and that is the substance of what a
    /// plugin may contribute to remediation rather than a detail of it: the explanation
    /// and the next action, for the operator to carry out. A repair that acts is a
    /// recipe, and a second way to declare one here would be the duplicate mechanism
    /// the contribution format exists to refuse.
    ///
    /// The meaning is the leading remedy's, because that is the field a declared remedy
    /// carries it in and the leading one is the likeliest account of what happened.
    fn problem(&self, faults: &[String]) -> Problem {
        let mut declared = self.remedies.iter();
        let leading = declared.next();
        let problem = Problem::new(
            CONTRIBUTED_FAILED,
            Severity::Error,
            format!("{} did not hold", self.of()),
            leading.map_or_else(
                || format!("{} declared no remedy that could be rendered", self.plugin),
                |one| one.means.clone(),
            ),
            leading.map_or_else(
                || {
                    Remedy::new(format!(
                        "Ask {}'s author what to do about {}",
                        self.plugin, self.check
                    ))
                },
                |one| one.remedy.clone(),
            ),
        )
        .in_state(State::Guided)
        .with_detail(faults.join("; "));
        declared.fold(problem, |problem, one| problem.or_try(one.remedy.clone()))
    }

    /// The finding for a check that could not be run at all.
    ///
    /// Unverified rather than failed, and never absent. Nothing has been established
    /// about the service either way, and reporting a check nobody could put as one the
    /// service failed would say the software is broken when the declaration is.
    fn unrun(&self, why: &str) -> Verdict {
        Verdict::Unverified {
            reason: format!("{} could not be run: {why}", self.of()),
            remedy: Remedy::new(format!(
                "Run this again once {} can be asked, or ask {}'s author about {}",
                self.service.as_deref().unwrap_or("the plugin's service"),
                self.plugin,
                self.check
            )),
        }
    }

    /// The check and the plugin it belongs to, which every line about it carries.
    fn of(&self) -> String {
        format!("{}, contributed by {}", self.check, self.plugin)
    }
}

/// What came back, in the terms the shared evaluator constrains.
///
/// The same shape a recording is read into, which is what lets one evaluator serve both
/// — a live answer and a recorded one are the same four facts, and two evaluators for
/// one vocabulary would be two things to keep in step.
///
/// Headers are folded to lower case and the first of a repeated one wins, matching how
/// the transport answers a question about one. A body is offered as a document where it
/// reads as one and as text either way, because an expectation may ask about either and
/// which it asks about is not this function's business.
fn answer(response: &http::Response) -> Answer {
    let mut headers: BTreeMap<String, String> = BTreeMap::new();
    for (name, value) in &response.headers {
        headers
            .entry(name.to_lowercase())
            .or_insert_with(|| value.clone());
    }
    Answer {
        status: response.status,
        headers,
        json: serde_json::from_str(&response.body).ok(),
        body_starts_with: Some(response.body.clone()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;
    use lemonfiber_fixtures::http::{Answer, Fake};
    use lemonfiber_plugin::Manifest;

    use super::{declared, CONTRIBUTED_FAILED};
    use crate::doctor::{examine, Category, Check, Finding, Narrowing, Overall, Verdict};
    use crate::error::{Problem, Remedy, State};
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
            .map(|manifest| declared(manifest, &answering(), http))
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
            Verdict::Pass { .. }
            | Verdict::Warn(_)
            | Verdict::Fail(_)
            | Verdict::Skipped { .. } => None,
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
                note: Some(
                    "komga:claimed, contributed by komga answered as it declares".to_owned()
                )
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
                note: Some(
                    "komga:claimed, contributed by komga answered as it declares".to_owned()
                )
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
            .map(|manifest| declared(manifest, &BTreeMap::new(), &saying(holding())))
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
}
