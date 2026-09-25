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
use lemonfiber_plugin::{Contribution, Expect};

use super::{Category, Check, Finding, Reported, Verdict};
use crate::error::codes::plugin::CONTRIBUTED_FAILED;
use crate::error::{Problem, Remedy, Severity, State};
use crate::plugin::judging::{judge, live, method};
use crate::plugin::recorded::Answer;
use crate::plugin::Installed;
use crate::ports::http::{self, Http, Method};

/// Every check an installed plugin adds to the register, ready to be run with the rest.
///
/// **Read off the record of what was installed rather than off the plugin's own
/// files.** The author's directory may be gone the moment an install is done, and a
/// diagnosis a month later is a question about this machine rather than about a
/// document. The record is what the install settled and is the one thing that survives,
/// which is also what stops a row changing under an operator between two runs.
///
/// `answering` says where each of the plugin's services can be reached, by the id the
/// manifest gave it. A service missing from it is one lemonfiber cannot ask, which is
/// a reason to report a check unrun rather than a reason to leave it out.
///
/// Nothing is returned for a plugin that contributes nothing, and nothing at all for no
/// plugins — which is what makes a run after a plugin is removed the same run as one on
/// a build that never saw it, rather than one with a gap where the plugin was.
#[must_use]
pub fn declared(
    installed: &Installed,
    answering: &BTreeMap<String, String>,
    http: &Arc<dyn Http>,
) -> Vec<Box<dyn Check>> {
    installed
        .contributions
        .iter()
        .filter(|entry| entry.at == extension::check())
        .map(|entry| {
            Box::new(Contributed {
                plugin: installed.plugin.clone(),
                check: entry.id.clone(),
                title: entry.title.clone().unwrap_or(entry.id.clone()),
                category: family(entry).unwrap_or(UNPLACEABLE),
                service: entry.service.clone(),
                budget: bounded(entry.timeout_s),
                asks: asked(entry, answering),
                remedies: remedies(installed, &entry.id),
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
fn asked(entry: &Contribution, answering: &BTreeMap<String, String>) -> Asks {
    if family(entry).is_none() {
        return Asks::Nothing(format!(
            "{} is not a family the doctor has, so there is nowhere to report it",
            entry.category.as_deref().unwrap_or("nothing")
        ));
    }
    let Some(service) = entry.service.clone() else {
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
    let Some(method) = method(&request.method) else {
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

/// The remedies this same plugin declared for this check, in the order it declared them.
///
/// Text, in the shape the error model already has, and that is the whole of what a
/// contributed remedy is. A repair that acts is a recipe and is declared as one; there
/// is nothing here that could run.
fn remedies(installed: &Installed, check: &str) -> Vec<Guided> {
    installed
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
            origin: self.origin(),
        })
    }

    async fn run(&self) -> Vec<Finding> {
        vec![self.found(self.verdict().await)]
    }
}

impl Contributed {
    /// Whose row this is: the plugin that declared it, named.
    fn origin(&self) -> crate::origin::Origin {
        crate::origin::Origin::Plugin {
            named: self.plugin.clone(),
        }
    }

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
            origin: self.origin(),
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
            Ok(response) => self.judged(expect, &live(&response)),
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

#[cfg(test)]
mod tests;
