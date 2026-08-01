//! Checks that prove things rather than assuming them.
//!
//! Every check is independent, bounded by its own timeout, and returns a finding
//! that carries a remedy. Checks are values in a collection rather than
//! branches in a function, so one that hangs or fails cannot take the others
//! with it.
//!
//! "Could not check" is a distinct variant of the finding type rather than a
//! severity value, so it cannot accidentally render as "passed" — which is the
//! failure that would make the whole feature dishonest.
//!
//! An error inside a check surfaces as a check error, never as a finding about
//! the stack.
//!
//! See `.docs/architecture/module-layout.md`.

pub mod credentials;
pub mod environment;
pub mod guides;
pub mod indexer;
pub mod storage;
pub mod vpn;

use std::time::Duration;

use async_trait::async_trait;
use serde::Serialize;

use crate::error::{Problem, Remedy};
use crate::model::DoctorReport;

/// The family a check belongs to, so a run can be narrowed to one of them.
///
/// These are the diagnostic categories the product recognises; the checks that
/// fill each one arrive over time, so a category may name more than lemonfiber
/// can yet establish.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    /// Docker present, the daemon reachable, the platform understood.
    Environment,
    /// The data root: reachable, writable, one filesystem, room to grow.
    Storage,
    /// Ports free, bindings matching policy, services reachable.
    Network,
    /// Torrent traffic genuinely leaves through the tunnel.
    Vpn,
    /// Each credential still valid.
    Credentials,
    /// Health, crash loops, version skew, the wiring between services.
    Services,
    /// Provider quota, subscription validity, indexer responsiveness.
    Providers,
    /// Stuck items, repeated import failures, orphaned downloads.
    Queue,
    /// Drift from lemonfiber-managed state, permissions, manifest validity.
    Config,
}

impl Category {
    /// The category as the operator names it to `--only`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Environment => "environment",
            Self::Storage => "storage",
            Self::Network => "network",
            Self::Vpn => "vpn",
            Self::Credentials => "credentials",
            Self::Services => "services",
            Self::Providers => "providers",
            Self::Queue => "queue",
            Self::Config => "config",
        }
    }

    /// The category an operator named, when it is one lemonfiber knows.
    ///
    /// An unknown name is `None` rather than a silent empty run, so a surface can
    /// tell the operator they mistyped rather than reporting that nothing was
    /// wrong.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        [
            Self::Environment,
            Self::Storage,
            Self::Network,
            Self::Vpn,
            Self::Credentials,
            Self::Services,
            Self::Providers,
            Self::Queue,
            Self::Config,
        ]
        .into_iter()
        .find(|category| category.as_str() == name)
    }
}

/// How a single check turned out.
///
/// "Could not check" (`Unverified`) is its own variant rather than a level of
/// severity, so a check that could not run can never be mistaken for one that
/// passed — the dishonesty this whole subsystem exists to prevent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Verdict {
    /// Verified working, with the evidence worth showing.
    Pass {
        /// What was observed, where stating it helps — an address, a port.
        note: Option<String>,
    },
    /// Working, but degraded or risky.
    Warn(Problem),
    /// Not working.
    Fail(Problem),
    /// Could not be established. Never a pass.
    Unverified {
        /// Why it could not be determined.
        reason: String,
        /// What the operator can do to get an answer.
        remedy: Remedy,
    },
    /// A prerequisite was absent, so the check did not apply.
    Skipped {
        /// Why the check did not apply.
        reason: String,
    },
}

/// One thing a check established, and how it turned out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    /// A stable identifier for the thing checked, such as `vpn.egress-match`.
    pub check: String,
    /// The family this belongs to.
    pub category: Category,
    /// The one-line summary of what was checked.
    pub title: String,
    /// How it turned out.
    pub verdict: Verdict,
}

impl Finding {
    /// A finding in a given category. Each check module wraps this with its own
    /// category bound, so the shape of a finding is written once here rather than
    /// re-spelled per module.
    #[must_use]
    pub fn in_category(category: Category, check: &str, title: &str, verdict: Verdict) -> Self {
        Self {
            check: check.to_owned(),
            category,
            title: title.to_owned(),
            verdict,
        }
    }
}

/// What a run's findings amount to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Overall {
    /// Everything that ran passed.
    Healthy,
    /// Warnings, but nothing broken.
    Degraded,
    /// Something is broken.
    Broken,
    /// Something could not be determined, so health is not a settled fact.
    Unknown,
}

/// How long a single check may run before it is abandoned as unverified.
///
/// Generous enough for a container command over a busy daemon, bounded because a
/// wait with no end is indistinguishable from a hang.
const CHECK_BUDGET: Duration = Duration::from_secs(15);

/// A single diagnostic, run in isolation from every other.
///
/// A check reports what it could not determine as an unverified finding and
/// never returns an error: one check's trouble must not become a finding about
/// the stack, nor stop the others running. What a check needs to do its work it
/// holds, rather than reaching for a shared context, so the checks stay values
/// in a list.
#[async_trait]
pub trait Check: Send + Sync {
    /// The family this check belongs to, so a run can be narrowed to it.
    fn category(&self) -> Category;

    /// How long this check may run before it is abandoned.
    ///
    /// The default suits a check bounded by a container command; a filesystem
    /// check over a slow NAS would want longer.
    fn budget(&self) -> Duration {
        CHECK_BUDGET
    }

    /// Establish the findings empirically.
    async fn run(&self) -> Vec<Finding>;
}

/// Run the checks that match, bound each by its own budget, and sum the result.
///
/// Naming a category runs only that one. Each check is awaited under a timeout,
/// so a wedged command becomes an unverified finding rather than a hung suite;
/// because the checks are values in a list, one timing out has no bearing on the
/// next.
pub async fn examine(checks: &[Box<dyn Check>], only: Option<Category>) -> DoctorReport {
    // The checks are independent I/O — process spawns, container execs, HTTP — so
    // they run at once rather than in series: a run's wall-clock then tracks the
    // slowest check, not their sum, which matters most on the struggling stack an
    // operator reaches for `doctor` against. `join_all` preserves the checks' order,
    // so the report reads the same as when they ran one at a time; each is still
    // bounded by its own budget, so one hanging has no bearing on the rest.
    let selected = checks
        .iter()
        .filter(|check| only.is_none_or(|wanted| wanted == check.category()));
    let findings: Vec<Finding> = futures_util::future::join_all(selected.map(|check| async move {
        match tokio::time::timeout(check.budget(), check.run()).await {
            Ok(produced) => produced,
            Err(_elapsed) => vec![timed_out(check.as_ref())],
        }
    }))
    .await
    .into_iter()
    .flatten()
    .collect();
    DoctorReport {
        overall: overall(&findings),
        findings,
    }
}

/// The finding a check becomes when it does not answer within its budget.
fn timed_out(check: &dyn Check) -> Finding {
    let seconds = check.budget().as_secs();
    Finding {
        check: check.category().as_str().to_owned(),
        category: check.category(),
        title: "Check timed out".to_owned(),
        verdict: Verdict::Unverified {
            reason: format!("did not finish within {seconds} seconds"),
            remedy: Remedy::new(
                "Run it again; if it keeps timing out, the engine or a service is not responding",
            ),
        },
    }
}

/// Reduce findings to one word.
///
/// Highest consequence wins, and it is never an average — one leaking VPN among
/// eighteen passes is not eighteen-nineteenths healthy. An unverified finding
/// keeps the answer out of `healthy`: a thing that could not be checked is not a
/// thing that passed, and a run that established nothing is `unknown` rather than
/// a comfortable green.
fn overall(findings: &[Finding]) -> Overall {
    let mut passed = false;
    let mut warned = false;
    let mut unverified = false;
    for finding in findings {
        match finding.verdict {
            Verdict::Fail(_) => return Overall::Broken,
            Verdict::Warn(_) => warned = true,
            Verdict::Unverified { .. } => unverified = true,
            Verdict::Pass { .. } => passed = true,
            Verdict::Skipped { .. } => {}
        }
    }
    if warned {
        Overall::Degraded
    } else if unverified {
        Overall::Unknown
    } else if passed {
        Overall::Healthy
    } else {
        Overall::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::{examine, Category, Check, Finding, Overall, Verdict};
    use crate::error::{Code, Problem, Remedy, Severity};
    use async_trait::async_trait;
    use std::time::Duration;

    const CODE: Code = Code::new("TEST-1");

    /// A check that answers with exactly the findings it was given.
    struct Fixed {
        category: Category,
        findings: Vec<Finding>,
    }

    #[async_trait]
    impl Check for Fixed {
        fn category(&self) -> Category {
            self.category
        }
        async fn run(&self) -> Vec<Finding> {
            self.findings.clone()
        }
    }

    /// A check that never answers, to prove the timeout turns it into a finding.
    struct Hanging;

    #[async_trait]
    impl Check for Hanging {
        fn category(&self) -> Category {
            Category::Vpn
        }
        fn budget(&self) -> Duration {
            Duration::from_secs(15)
        }
        async fn run(&self) -> Vec<Finding> {
            std::future::pending::<()>().await;
            Vec::new()
        }
    }

    fn finding(title: &str, verdict: Verdict) -> Finding {
        Finding {
            check: "test".to_owned(),
            category: Category::Vpn,
            title: title.to_owned(),
            verdict,
        }
    }

    fn problem() -> Problem {
        Problem::new(
            CODE,
            Severity::Error,
            "It broke",
            "The thing did not happen",
            Remedy::new("Try again"),
        )
    }

    fn fixed(category: Category, findings: Vec<Finding>) -> Box<dyn Check> {
        Box::new(Fixed { category, findings })
    }

    #[tokio::test]
    async fn all_passing_is_healthy() {
        let checks = vec![fixed(
            Category::Vpn,
            vec![finding("egress", Verdict::Pass { note: None })],
        )];
        let report = examine(&checks, None).await;
        assert_eq!(report.overall, Overall::Healthy);
        assert_eq!(report.findings.len(), 1);
    }

    #[tokio::test]
    async fn a_warning_degrades_but_does_not_break() {
        let checks = vec![fixed(
            Category::Vpn,
            vec![
                finding("egress", Verdict::Pass { note: None }),
                finding("port", Verdict::Warn(problem())),
            ],
        )];
        assert_eq!(examine(&checks, None).await.overall, Overall::Degraded);
    }

    #[tokio::test]
    async fn any_failure_breaks_the_whole() {
        let checks = vec![fixed(
            Category::Vpn,
            vec![
                finding("egress", Verdict::Fail(problem())),
                finding("other", Verdict::Pass { note: None }),
            ],
        )];
        assert_eq!(examine(&checks, None).await.overall, Overall::Broken);
    }

    #[tokio::test]
    async fn an_unverified_check_keeps_a_pass_out_of_healthy() {
        // The killswitch case: egress verified, but something could not be
        // established, so the run must not claim health.
        let checks = vec![fixed(
            Category::Vpn,
            vec![
                finding("egress", Verdict::Pass { note: None }),
                finding(
                    "killswitch",
                    Verdict::Unverified {
                        reason: "not tested".to_owned(),
                        remedy: Remedy::new("run it"),
                    },
                ),
            ],
        )];
        assert_eq!(examine(&checks, None).await.overall, Overall::Unknown);
    }

    #[tokio::test]
    async fn a_run_that_establishes_nothing_is_unknown_not_healthy() {
        let checks = vec![fixed(
            Category::Vpn,
            vec![finding(
                "vpn",
                Verdict::Skipped {
                    reason: "no VPN configured".to_owned(),
                },
            )],
        )];
        assert_eq!(examine(&checks, None).await.overall, Overall::Unknown);
    }

    #[tokio::test]
    async fn naming_a_category_runs_only_that_one() {
        let checks = vec![
            fixed(
                Category::Vpn,
                vec![finding("egress", Verdict::Pass { note: None })],
            ),
            fixed(
                Category::Storage,
                vec![finding("disk", Verdict::Fail(problem()))],
            ),
        ];
        let report = examine(&checks, Some(Category::Vpn)).await;
        // The storage failure is not run, so it cannot break the result.
        assert_eq!(report.overall, Overall::Healthy);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings.first().map(|found| found.category),
            Some(Category::Vpn)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_check_that_will_not_answer_becomes_unverified_rather_than_hanging() {
        let checks: Vec<Box<dyn Check>> = vec![Box::new(Hanging)];
        let report = examine(&checks, None).await;
        assert_eq!(report.overall, Overall::Unknown);
        let verdict = report.findings.first().map(|found| &found.verdict);
        assert!(matches!(verdict, Some(Verdict::Unverified { .. })));
    }

    #[test]
    fn every_category_survives_a_round_trip_through_its_name() {
        for category in [
            Category::Environment,
            Category::Storage,
            Category::Network,
            Category::Vpn,
            Category::Credentials,
            Category::Services,
            Category::Providers,
            Category::Queue,
            Category::Config,
        ] {
            assert_eq!(Category::parse(category.as_str()), Some(category));
        }
    }

    #[test]
    fn a_name_lemonfiber_does_not_know_is_not_a_category() {
        assert_eq!(Category::parse("nonsense"), None);
    }

    /// The machine-readable output is a versioned contract, so the exact words
    /// on the wire are pinned here rather than left to chance.
    #[test]
    fn every_outcome_has_a_name_on_the_wire() {
        use crate::model::DoctorReport;
        let report = DoctorReport {
            overall: Overall::Broken,
            findings: vec![
                finding(
                    "egress",
                    Verdict::Pass {
                        note: Some("185.65.1.1".to_owned()),
                    },
                ),
                finding("port", Verdict::Warn(problem())),
                finding("leak", Verdict::Fail(problem())),
                finding(
                    "killswitch",
                    Verdict::Unverified {
                        reason: "untested".to_owned(),
                        remedy: Remedy::new("run it"),
                    },
                ),
                finding(
                    "forwarded-port",
                    Verdict::Skipped {
                        reason: "no port forwarding".to_owned(),
                    },
                ),
            ],
        };
        let json = serde_json::to_string(&report).unwrap_or_default();
        assert!(json.contains(r#""overall":"broken""#));
        assert!(json.contains(r#""category":"vpn""#));
        for outcome in ["pass", "warn", "fail", "unverified", "skipped"] {
            assert!(
                json.contains(&format!(r#""outcome":"{outcome}""#)),
                "{outcome} should name itself in {json}"
            );
        }
    }

    #[test]
    fn a_category_serialises_to_the_name_the_operator_types() {
        for category in [
            Category::Environment,
            Category::Storage,
            Category::Network,
            Category::Vpn,
            Category::Credentials,
            Category::Services,
            Category::Providers,
            Category::Queue,
            Category::Config,
        ] {
            assert_eq!(
                serde_json::to_string(&category).unwrap_or_default(),
                format!("\"{}\"", category.as_str())
            );
        }
    }

    #[test]
    fn an_overall_serialises_to_a_single_word() {
        for (overall, word) in [
            (Overall::Healthy, "healthy"),
            (Overall::Degraded, "degraded"),
            (Overall::Broken, "broken"),
            (Overall::Unknown, "unknown"),
        ] {
            assert_eq!(
                serde_json::to_string(&overall).unwrap_or_default(),
                format!("\"{word}\"")
            );
        }
    }
}
