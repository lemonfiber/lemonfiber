use async_trait::async_trait;
use std::time::Duration;

use super::{attributed, examine};
use crate::doctor::fixtures::{finding, problem};
use crate::doctor::{Category, Check, Finding, Narrowing, Overall, Verdict};
use crate::error::{Problem, Remedy};

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

/// A plugin's check that never answers, and says whose it is.
struct HangingAndAttributed;

#[async_trait]
impl Check for HangingAndAttributed {
    fn category(&self) -> Category {
        Category::Services
    }
    fn budget(&self) -> Duration {
        Duration::from_secs(15)
    }
    fn reports(&self) -> Option<crate::doctor::Reported> {
        Some(crate::doctor::Reported {
            check: "komga:libraries".to_owned(),
            service: Some("komga".to_owned()),
            origin: crate::origin::Origin::Plugin {
                named: "komga".to_owned(),
            },
        })
    }
    async fn run(&self) -> Vec<Finding> {
        std::future::pending::<()>().await;
        Vec::new()
    }
}

/// Two services, one standing on the other, as a manifest declares them.
fn stack() -> Vec<lemonfiber_manifest::Service> {
    let text = r#"
schema_version = 1
stack_version = "1.0.0"
min_cli_version = "0.1.0"

[[profile]]
id = "torrent"
name = "Torrents"
description = "Downloading over a tunnel"

[[form]]
id = "dl"
name = "Download"
description = "Fetch a link"
profiles = ["torrent"]

[[service]]
id = "gluetun"
name = "Gluetun"
profile = "torrent"
image = "qmcgaw/gluetun"
tag = "v3.40.0"
criticality = "critical"
license = "MIT"
upstream = "https://github.com/qdm12/gluetun"
last_release = "2026-01-01"
describes = "The tunnel"
without_it = "Nothing downloads over a tunnel"

[[service]]
id = "qbittorrent"
name = "qBittorrent"
profile = "torrent"
image = "lscr.io/linuxserver/qbittorrent"
tag = "5.0.3"
criticality = "core"
license = "GPL-2.0"
upstream = "https://github.com/qbittorrent/qBittorrent"
last_release = "2026-01-01"
describes = "The client"
without_it = "Nothing is downloaded"
depends_on = ["gluetun"]
"#;
    lemonfiber_manifest::Manifest::from_toml(text)
        .map(|manifest| manifest.services)
        .unwrap_or_default()
}

/// A service that cannot work because the thing underneath it is down is one problem
/// with the thing underneath, not two — which is the difference between a report an
/// operator reads and a list they give up on.
#[test]
fn a_finding_downstream_of_another_says_which_one_explains_it() {
    let tunnel =
        Finding::in_category(Category::Vpn, "vpn.up", "The tunnel", failing()).about("gluetun");
    let client = Finding::in_category(
        Category::Credentials,
        "credentials.qbittorrent",
        "The client",
        failing(),
    )
    .about("qbittorrent");

    let linked = attributed(vec![tunnel, client], &stack());
    assert_eq!(
        linked
            .iter()
            .map(|f| f.caused_by.clone())
            .collect::<Vec<_>>(),
        vec![None, Some("vpn.up".to_owned())],
        "the one underneath explains the one on top, and nothing explains the tunnel"
    );
}

/// A dependency that is working explains nothing. Attributing to it would tell the
/// operator to go and look at a service that is behaving perfectly.
#[test]
fn a_dependency_that_is_passing_is_not_offered_as_an_explanation() {
    let tunnel = Finding::in_category(
        Category::Vpn,
        "vpn.up",
        "The tunnel",
        Verdict::Pass { note: None },
    )
    .about("gluetun");
    let client = Finding::in_category(
        Category::Credentials,
        "credentials.qbittorrent",
        "The client",
        failing(),
    )
    .about("qbittorrent");

    let linked = attributed(vec![tunnel, client], &stack());
    // Both survived attribution. An `all` over an empty list is true, so without
    // this the check below would pass for a function that dropped everything.
    assert_eq!(linked.len(), 2, "{linked:?}");
    assert!(linked.iter().all(|finding| finding.caused_by.is_none()));
}

/// Checks about the machine rather than about something running on it have no service
/// to be downstream of, and are left exactly as they were.
#[test]
fn a_finding_about_no_service_is_left_alone() {
    let environment = Finding::in_category(
        Category::Environment,
        "environment.compose",
        "Compose",
        failing(),
    );
    let linked = attributed(vec![environment.clone()], &stack());
    assert_eq!(linked, vec![environment]);
}

/// A failing verdict, for the tests above.
fn failing() -> Verdict {
    Verdict::Fail(Problem::new(
        crate::error::Code::new("TEST-1"),
        crate::error::Severity::Error,
        "it is not working",
        "it means what it says",
        Remedy::new("put it right"),
    ))
}

/// A finding reporting under the identifier a run can be narrowed to.
fn identified(category: Category, check: &str, verdict: Verdict) -> Finding {
    Finding::in_category(category, check, "What was checked", verdict)
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
    let report = examine(&checks, &Narrowing::Suite).await;
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
    assert_eq!(
        examine(&checks, &Narrowing::Suite).await.overall,
        Overall::Degraded
    );
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
    assert_eq!(
        examine(&checks, &Narrowing::Suite).await.overall,
        Overall::Broken
    );
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
    assert_eq!(
        examine(&checks, &Narrowing::Suite).await.overall,
        Overall::Unknown
    );
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
    assert_eq!(
        examine(&checks, &Narrowing::Suite).await.overall,
        Overall::Unknown
    );
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
    let report = examine(&checks, &Narrowing::Category(Category::Vpn)).await;
    // The storage failure is not run, so it cannot break the result.
    assert_eq!(report.overall, Overall::Healthy);
    assert_eq!(report.findings.len(), 1);
    assert_eq!(
        report.findings.first().map(|found| found.category),
        Some(Category::Vpn)
    );
}

/// The suite, a family, or one check: the third of those is named by the id the
/// finding itself carries, so a report's own words are what goes back in.
#[tokio::test]
async fn naming_one_check_reports_only_that_check() {
    let checks = vec![
        fixed(
            Category::Storage,
            vec![
                identified(
                    Category::Storage,
                    "storage.space",
                    Verdict::Pass { note: None },
                ),
                identified(
                    Category::Storage,
                    "storage.hardlinks",
                    Verdict::Fail(problem()),
                ),
            ],
        ),
        fixed(
            Category::Vpn,
            vec![identified(
                Category::Vpn,
                "vpn.tunnel",
                Verdict::Fail(problem()),
            )],
        ),
    ];

    let report = examine(&checks, &Narrowing::Check("storage.space".to_owned())).await;

    assert_eq!(
        report
            .findings
            .iter()
            .map(|found| found.check.as_str())
            .collect::<Vec<&str>>(),
        vec!["storage.space"]
    );
    // Neither the neighbour in its own family nor the failure in another is
    // graded here: what was asked for is what the verdict is about.
    assert_eq!(report.overall, Overall::Healthy);
}

#[tokio::test(start_paused = true)]
async fn a_check_that_will_not_answer_becomes_unverified_rather_than_hanging() {
    let checks: Vec<Box<dyn Check>> = vec![Box::new(Hanging)];
    let report = examine(&checks, &Narrowing::Suite).await;
    assert_eq!(report.overall, Overall::Unknown);
    let verdict = report.findings.first().map(|found| &found.verdict);
    assert!(matches!(verdict, Some(Verdict::Unverified { .. })));
    assert_eq!(
        report.findings.first().map(|found| &found.origin),
        Some(&crate::origin::Origin::Bundled),
        "a check of this build's own that says nothing is this build's own"
    );
}

/// A plugin's check abandoned at its budget is written as that plugin's check not
/// having run — its own id, its own service, and whose it is — rather than as a row
/// of this build's.
#[tokio::test(start_paused = true)]
async fn a_plugin_s_check_that_will_not_answer_is_still_the_plugin_s() {
    let checks: Vec<Box<dyn Check>> = vec![Box::new(HangingAndAttributed)];
    let report = examine(&checks, &Narrowing::Suite).await;
    let found = report.findings.first();
    assert_eq!(found.map(|one| one.check.as_str()), Some("komga:libraries"));
    assert_eq!(
        found.map(|one| &one.origin),
        Some(&crate::origin::Origin::Plugin {
            named: "komga".to_owned()
        })
    );
}
