//! The proofs an installed plugin declares that run the image's own program, run by the
//! doctor on its own cadence: once, and again only when what the run depends on moves.

mod kept;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use super::{Category, Check, Finding, Reported, Verdict};
use crate::error::codes::plugin::PROOF_RUN_FAILED;
use crate::error::{Problem, Remedy, State};
use crate::plugin::program::{limit, performed, starting_digest, verdict, Operator, Starting};
use crate::plugin::{Installed, Placed, RunProof};
use crate::ports::program::Program;

pub(crate) use kept::{Kept, Key};

/// How long a run proof's check may take beyond its run: making and removing the
/// container and copying its starting files.
const BESIDE_THE_RUN: Duration = Duration::from_secs(30);

/// The family a run proof is filed under.
const FILED: Category = Category::Services;

/// What every run proof's check is built with, beside the proof itself.
#[derive(Clone)]
pub(crate) struct Running {
    /// How the image's own program is run.
    pub(crate) program: Arc<dyn Program>,
    /// What the entry writes as the time zone and the operator's ids.
    pub(crate) operator: Operator,
    /// Where the starting files are kept, a directory for each plugin beneath it.
    pub(crate) starting: Option<PathBuf>,
    /// Where the verdicts are kept across restarts, where anywhere is configured.
    pub(crate) kept: Option<PathBuf>,
    /// The moment this run is stamped with.
    pub(crate) now: String,
}

/// A check for every run proof `installed` declares.
pub(crate) fn declared(installed: &Installed, running: &Running) -> Vec<Box<dyn Check>> {
    installed
        .run_proofs
        .iter()
        .filter_map(|proof| {
            let placed = installed
                .services
                .iter()
                .find(|one| one.service == proof.service)?;
            Some(Box::new(Proved {
                plugin: installed.plugin.clone(),
                proof: proof.clone(),
                placed: placed.clone(),
                running: running.clone(),
            }) as Box<dyn Check>)
        })
        .collect()
}

/// One run proof of one installed plugin.
struct Proved {
    plugin: String,
    proof: RunProof,
    placed: Placed,
    running: Running,
}

#[async_trait]
impl Check for Proved {
    fn category(&self) -> Category {
        FILED
    }

    fn budget(&self) -> Duration {
        Duration::from_secs(u64::from(limit(&self.proof.run))) + BESIDE_THE_RUN
    }

    fn reports(&self) -> Option<Reported> {
        Some(Reported {
            check: self.check(),
            service: Some(self.proof.service.clone()),
            origin: self.origin(),
        })
    }

    async fn run(&self) -> Vec<Finding> {
        vec![self.found(self.verdict().await)]
    }
}

impl Proved {
    /// What its finding is reported against.
    fn check(&self) -> String {
        format!("{}:{}", self.plugin, self.proof.proof)
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
            title: self.proof.title.clone(),
            verdict,
            service: Some(self.proof.service.clone()),
            service_name: None,
            caused_by: None,
            said: None,
            onset: None,
            origin: self.origin(),
            disruptive: None,
        }
    }

    /// The verdict kept for what the run depends on now, or a fresh run's where none is.
    async fn verdict(&self) -> Verdict {
        let root = self
            .running
            .starting
            .as_ref()
            .map(|kept| kept.join(&self.plugin));
        let starting = root.as_deref().map(|root| Starting {
            root,
            service: &self.proof.service,
        });
        let digest = match starting_digest(starting) {
            Ok(digest) => digest,
            Err(why) => return self.unrun(&why),
        };
        if self
            .placed
            .starting
            .as_ref()
            .is_some_and(|kept| *kept != digest)
        {
            return self
                .unrun("the starting files kept when it was installed are not as they were then");
        }
        let key = Key::of(&self.placed, &self.proof, &self.running.operator, digest);
        let held = kept::read(self.running.kept.as_deref());
        if let Some(earlier) = held.iter().find(|one| {
            one.plugin == self.plugin && one.proof == self.proof.proof && one.key == key
        }) {
            return self.reported(&earlier.verdict, &earlier.reached);
        }
        let came = performed(
            self.running.program.as_ref(),
            &self.placed,
            &self.proof.run,
            &self.running.operator,
            starting,
        )
        .await;
        let reached = verdict(&came, &self.proof.expect);
        if !matches!(reached, crate::plugin::Verdict::Unproven { .. }) {
            kept::keep(
                self.running.kept.as_deref(),
                held,
                Kept {
                    plugin: self.plugin.clone(),
                    proof: self.proof.proof.clone(),
                    key,
                    verdict: reached.clone(),
                    reached: self.running.now.clone(),
                },
            );
        }
        self.reported(&reached, &self.running.now)
    }

    /// A proof's verdict as the doctor reports it, with when it was reached.
    fn reported(&self, reached: &crate::plugin::Verdict, at: &str) -> Verdict {
        match reached {
            crate::plugin::Verdict::Passed | crate::plugin::Verdict::FailingAsDeclared { .. } => {
                Verdict::Pass {
                    note: Some(format!("{} ran as it declares, reached at {at}", self.of())),
                }
            }
            crate::plugin::Verdict::Failed { faults } => Verdict::Fail(
                Problem::new(
                    PROOF_RUN_FAILED,
                    format!("{} did not hold, reached at {at}", self.of()),
                    format!(
                        "{} ran the image's own program, and it did not come to what the proof \
                         declares.",
                        self.of()
                    ),
                    Remedy::new(format!(
                        "Prove {} again, and take what it says up with its author",
                        self.plugin
                    ))
                    .with_detail(format!("lemonfiber plugin prove {}", self.plugin)),
                )
                .in_state(State::Guided)
                .with_detail(faults.join("; ")),
            ),
            crate::plugin::Verdict::Unproven { why } => self.unrun(why),
        }
    }

    /// The finding for a run that could not be made.
    fn unrun(&self, why: &str) -> Verdict {
        Verdict::Unverified {
            reason: format!("{} could not be run: {why}", self.of()),
            remedy: Remedy::new(format!(
                "Run the doctor again once the engine holds {}@{}",
                self.placed.image, self.placed.digest
            )),
        }
    }

    /// The proof and the plugin it belongs to.
    fn of(&self) -> String {
        format!("{}, proved by {}", self.proof.proof, self.plugin)
    }
}

#[cfg(test)]
mod tests;
