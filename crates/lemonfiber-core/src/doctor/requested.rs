//! The proofs an installed plugin declares that ask one of its services a question, run
//! by the doctor as checks like any other.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_plugin::extension;

use super::asking::{asked, put, Asks};
use super::proved::reproving;
use super::{Category, Check, Finding, Reported, Verdict};
use crate::error::codes::plugin::PROOF_ASKED_FAILED;
use crate::error::{Problem, Remedy, State};
use crate::plugin::Installed;
use crate::ports::http::Http;

/// The family a request proof is filed under.
const FILED: Category = Category::Services;

/// A check for every request proof `installed` declares, asked where `answering` says
/// each of its services answers.
pub(crate) fn declared(
    installed: &Installed,
    answering: &BTreeMap<String, String>,
    http: &Arc<dyn Http>,
) -> Vec<Box<dyn Check>> {
    installed
        .request_proofs
        .iter()
        .map(|proof| {
            Box::new(Requested {
                plugin: installed.plugin.clone(),
                proof: proof.proof.clone(),
                title: proof.title.clone(),
                service: proof.service.clone(),
                asks: asked(
                    Some(&proof.service),
                    Some(&proof.request),
                    Some(&proof.expect),
                    answering,
                ),
                http: Arc::clone(http),
            }) as Box<dyn Check>
        })
        .collect()
}

/// One request proof of one installed plugin.
struct Requested {
    plugin: String,
    proof: String,
    title: String,
    service: String,
    asks: Asks,
    http: Arc<dyn Http>,
}

#[async_trait]
impl Check for Requested {
    fn category(&self) -> Category {
        FILED
    }

    fn budget(&self) -> Duration {
        Duration::from_secs(u64::from(extension::CHECK_TIMEOUT.default))
    }

    fn reports(&self) -> Option<Reported> {
        Some(Reported {
            check: self.check(),
            service: Some(self.service.clone()),
            origin: self.origin(),
        })
    }

    async fn run(&self) -> Vec<Finding> {
        vec![self.found(self.verdict().await)]
    }
}

impl Requested {
    /// What its finding is reported against.
    fn check(&self) -> String {
        format!("{}:{}", self.plugin, self.proof)
    }

    /// Whose check this is.
    fn origin(&self) -> crate::origin::Origin {
        crate::origin::Origin::Plugin {
            named: self.plugin.clone(),
        }
    }

    /// The finding, attributed to the plugin that declared the proof.
    fn found(&self, verdict: Verdict) -> Finding {
        Finding {
            check: self.check(),
            category: FILED,
            title: self.title.clone(),
            verdict,
            service: Some(self.service.clone()),
            service_name: None,
            caused_by: None,
            said: None,
            onset: None,
            origin: self.origin(),
            disruptive: None,
        }
    }

    /// What asking the service came to.
    async fn verdict(&self) -> Verdict {
        match put(self.http.as_ref(), &self.asks).await {
            Err(why) => Verdict::Unverified {
                reason: format!("{} could not be asked: {why}", self.of()),
                remedy: Remedy::new(format!(
                    "Run the doctor again once {} answers",
                    self.service
                )),
            },
            Ok(faults) if faults.is_empty() => Verdict::Pass {
                note: Some(format!("{} answered as it declares", self.of())),
            },
            Ok(faults) => Verdict::Fail(
                Problem::new(
                    PROOF_ASKED_FAILED,
                    format!("{} did not hold", self.of()),
                    format!(
                        "{} asked {}, and the answer is not what the proof declares.",
                        self.of(),
                        self.service
                    ),
                    reproving(&self.plugin),
                )
                .in_state(State::Guided)
                .with_detail(faults.join("; ")),
            ),
        }
    }

    /// The proof and the plugin it belongs to.
    fn of(&self) -> String {
        format!("{}, proved by {}", self.proof, self.plugin)
    }
}

#[cfg(test)]
mod tests;
