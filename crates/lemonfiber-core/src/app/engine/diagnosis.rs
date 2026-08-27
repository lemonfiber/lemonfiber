//! Assembling the checks a diagnosis runs, and running them.
//!
//! Apart from the rest of the engine's errands because the assembling has a second caller:
//! a repair proves it worked by asking the very same checks again, and two lists would be
//! one list plus whichever check somebody forgot to add to the other.
//!
//! The pairing of running with acknowledging lives here too, for the same reason. A choice
//! the operator has already answered must read as answered wherever the checks are run,
//! and a caller that ran them without applying that would report a settled question as
//! freshly wrong.

use std::sync::Arc;

use crate::doctor::bindings::BindingsCheck;
use crate::doctor::credentials::CredentialsCheck;
use crate::doctor::environment::EnvironmentCheck;
use crate::doctor::guides::GuidesCheck;
use crate::doctor::headroom::HeadroomCheck;
use crate::doctor::indexer::IndexerCheck;
use crate::doctor::providers::ProvidersCheck;
use crate::doctor::releases::ReleasesCheck;
use crate::doctor::storage::StorageCheck;
use crate::doctor::vpn::VpnCheck;
use crate::doctor::wiring::WiringCheck;
use crate::doctor::{examine, Check, Finding, Narrowing, Verdict};
use crate::error::{Code, Diagnose, Problem, Remedy, Severity};
use crate::model::DoctorReport;
use crate::ports::service::{Indexers, UsenetAccounts};

use crate::app::targets::{committed_bytes, project_directory, servarr_targets};
use crate::app::Ctx;

/// Raised when a run is narrowed to a check nothing in this stack reports.
const NO_SUCH_CHECK: Code = Code::new("DIAG-1");

/// Run the diagnostic checks: the whole suite, one category, or one check.
///
/// The checks are assembled here rather than held on the context because each
/// needs a slice of it — the VPN check needs the engine, the resolved pair and
/// the operator's echo choice — and building them at the point of use keeps the
/// context a bag of capabilities rather than a registry of features.
///
/// Public as well as dispatched, because a caller that wants a diagnosis wants a
/// report: routing it through the command enum would hand back an outcome that
/// has to be destructured, with an arm for every answer it cannot be.
///
/// # Errors
///
/// Returns a [`Problem`] where the stack cannot be read, which is the one thing
/// the checks need before any of them can run, and where the run was narrowed to a
/// check nothing reports.
pub async fn diagnose(
    ctx: &Ctx,
    narrowing: &Narrowing,
    disruptive: bool,
) -> Result<DoctorReport, Box<Problem>> {
    let (manifest, checks) = assembled(ctx, disruptive).await?;
    let report = examined(ctx, &manifest.services, &checks, narrowing).await;
    answered(narrowing, report)
}

/// The report, or a refusal where a named check said nothing at all.
///
/// A check that could not run says so, and a check whose prerequisites are absent says
/// so too, so an empty report from a named check means the name matched none of them.
/// Reporting that as a healthy silence would settle a question nobody had asked.
fn answered(narrowing: &Narrowing, report: DoctorReport) -> Result<DoctorReport, Box<Problem>> {
    match narrowing.check() {
        Some(named) if report.findings.is_empty() => Err(Box::new(no_such_check(named))),
        _ => Ok(report),
    }
}

/// Why a named check was refused, and where the names that would work are written.
///
/// The names are not listed here. What a stack reports depends on what it is running —
/// one credential check per service it holds — so a list kept in this crate would be
/// right about somebody else's stack and wrong about the one in front of the operator.
fn no_such_check(named: &str) -> Problem {
    Problem::new(
        NO_SUCH_CHECK,
        Severity::Error,
        format!("No check on this stack reports as {named}"),
        "A check is named by the identifier its finding carries, and this run holds no \
         finding under that one. A report of nothing found would read as nothing wrong.",
        Remedy::new("Run the checks and name one this stack actually reports")
            .with_detail("lemonfiber doctor"),
    )
}

/// Run the checks and answer with what they found, acknowledged choices marked as such.
///
/// The one place that pairing happens. A choice the operator has already answered is
/// marked as answered rather than repeated — least of all the one about running with no
/// tunnel, which is the choice most likely to be deliberate and most tiresome repeated —
/// and a second caller that ran the checks without applying it would report a settled
/// question as freshly wrong. A repair proving its work is exactly such a caller.
pub(crate) async fn examined(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    checks: &[Box<dyn Check>],
    narrowing: &Narrowing,
) -> DoctorReport {
    let mut report = examine(checks, narrowing).await;
    report.findings =
        crate::doctor::acknowledged::suppressing(report.findings, &crate::app::accepted::load(ctx));
    // Attributed after the acknowledged ones are marked, so a choice the operator has
    // already answered is not offered as the explanation for anything else.
    report.findings = crate::doctor::attributed(report.findings, services);
    // Last, so a finding already explained by another service's trouble is quoted with
    // its own output rather than instead of it. Reading a service's output is not the
    // check's own business, which is why it happens here and not inside one.
    report.findings = quoted(ctx, report.findings).await;
    report
}

/// Every finding in trouble, carrying what its service said for itself lately.
///
/// A check can say a service is not answering; only the service can say why, and an
/// operator who has to go and fetch that has been handed a fault report rather than a
/// diagnosis.
///
/// Passing findings are left alone. Evidence for something that is working is noise,
/// and it would be the bulk of a healthy run — nineteen services' scrollback attached
/// to nineteen findings that all say the same thing.
///
/// One request for all of them rather than one each: the engine is asked for the
/// services in trouble together, and the lines are dealt back out by the service that
/// wrote them.
///
/// Withheld as they are gathered rather than as they are drawn. A service that fails
/// while authenticating says so with the credential in hand, and this is the one field
/// that carries such a sentence to three places at once: a report on a terminal, a
/// screen, and `said` on a finding served over HTTP. Redacting at a renderer would
/// leave the last of those untouched, and a bug report is pasted out of the first.
///
/// The same rule the same lines already take when they become an error's detail, so a
/// diagnosis and the failure beside it cannot answer differently about the same output.
/// It reads each line on its own and decides by that line's shape: one written as a
/// setting loses its value, and one written as a sentence keeps every word it has.
pub(crate) async fn quoted(ctx: &Ctx, findings: Vec<Finding>) -> Vec<Finding> {
    let troubled: Vec<String> = findings
        .iter()
        .filter(|finding| !matches!(finding.verdict, Verdict::Pass { .. }))
        .filter_map(|finding| finding.service.clone())
        .collect();

    if troubled.is_empty() {
        return findings;
    }

    let lines = super::lately(ctx, &troubled).await;
    findings
        .into_iter()
        .map(|finding| {
            let said: String = finding
                .service
                .as_deref()
                .filter(|_| !matches!(finding.verdict, Verdict::Pass { .. }))
                .map(|service| {
                    lines.iter().filter(|line| line.service == service).fold(
                        String::new(),
                        |mut said, line| {
                            said.push_str(&crate::config::store::withheld_text(&line.line));
                            said.push('\n');
                            said
                        },
                    )
                })
                .unwrap_or_default();

            // Absent rather than empty: a heading with nothing under it promises
            // evidence that is not there.
            Finding {
                said: (!said.is_empty()).then_some(said),
                ..finding
            }
        })
        .collect()
}

/// The checks this stack is examined by, built and ready to run.
///
/// Assembled apart from the running of them because a repair has to ask the very same
/// checks again to prove it worked, and two lists would be one list plus whichever check
/// somebody forgot to add to the other.
///
/// # Errors
///
/// Returns a [`Problem`] where the stack cannot be read, which is the one thing every
/// check needs before any of them can run.
pub(crate) async fn assembled(
    ctx: &Ctx,
    disruptive: bool,
) -> Result<(lemonfiber_manifest::Manifest, Vec<Box<dyn Check>>), Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;

    let environment = EnvironmentCheck::new(ctx.runner.clone());
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    // What the download clients still have to write, so the free-space finding
    // projects exhaustion from the queue rather than only warning on a floor.
    // Resolved from the same running-stack services the credentials check reaches;
    // a client that will not answer contributes nothing, so a stack whose clients
    // are all quiet reads as zero committed and the finding guards the raw free
    // space.
    let committed = committed_bytes(ctx, &manifest.services, project.as_deref()).await;
    let storage = StorageCheck::new(
        ctx.filesystem.clone(),
        ctx.settings.data_root.clone(),
        ctx.settings.storage_state.clone(),
        ctx.environment,
        ctx.settings.service_user,
        Some(committed),
    );
    let vpn = VpnCheck::new(
        ctx.engine.clone(),
        ctx.settings.project.clone(),
        &manifest,
        crate::doctor::vpn::Asked {
            protocols: ctx.settings.protocols,
            echo: ctx.settings.ip_echo.clone(),
            // Asked here rather than inside the check: that one speaks to
            // containers, and this is a service's own API.
            listening: crate::app::forwarding::listening_port(ctx, &manifest, project.as_deref())
                .await,
            port_forward: ctx.settings.port_forward.clone(),
            disruptive,
            // Built here because a client's credentials are a service's own business,
            // and the check speaks to containers. Absent where it could not be
            // authenticated to, which is a client to leave alone.
            client: crate::app::targets::torrent_client(
                ctx,
                &crate::app::targets::download_targets(&manifest.services, project.as_deref()),
            ),
        },
    );
    let credentials = CredentialsCheck::new(
        ctx.http.clone(),
        ctx.filesystem.clone(),
        servarr_targets(&manifest.services, project.as_deref()),
    );
    // The indexer the operator gave at setup is re-proven the same way it was
    // first proven — the shared validator over the same HTTP seam — so a key that
    // has since rotted is a finding rather than an empty search weeks on.
    let indexer = IndexerCheck::new(
        Arc::new(crate::validate::Live::new(ctx.http.clone())),
        ctx.settings.indexer.clone(),
    );
    // Whether the upstream quality guides can be reached, so a sync that would come
    // back empty — leaving the profiles in place stale rather than unconfigured — is
    // reported rather than silently missed.
    let guides = GuidesCheck::new(ctx.http.clone());
    // Whether the disk can plausibly hold a library at the chosen quality, projected
    // against the free space so an implausible choice is caught before it fills the
    // disk. The hungriest preset in force is the basis — the one that stresses the
    // disk most. An unreadable or unset choice falls back to the default rather than
    // failing the run.
    let headroom = HeadroomCheck::new(
        ctx.filesystem.clone(),
        ctx.settings.data_root.clone(),
        crate::app::quality::most_demanding_or_default(ctx),
    );
    // Whether the chosen quality actually finds releases — a demanding preset can ask
    // for what the indexers do not carry, which reads as an indexer fault unless the two
    // are told apart. It searches for wanted content live, so it only does so on a
    // disruptive run; otherwise it reports skipped.
    let releases = ReleasesCheck::new(
        ctx.http.clone(),
        ctx.filesystem.clone(),
        servarr_targets(&manifest.services, project.as_deref()),
        disruptive,
    );
    // What the accounts underneath the stack have left, read from the services that
    // use them — the download client that pulls through the Usenet accounts and the
    // aggregator that queries the indexers, both of which keep their own records. So
    // this costs the providers nothing: a check that spent the quota it measures would
    // help cause the outage it is there to warn about.
    let providers = ProvidersCheck::new(
        crate::app::targets::usenet_client(ctx, &manifest.services, project.as_deref())
            .await
            .map(|client| Arc::new(client) as Arc<dyn UsenetAccounts>),
        crate::app::targets::indexer_aggregator(ctx, &manifest.services, project.as_deref())
            .await
            .map(|aggregator| Arc::new(aggregator) as Arc<dyn Indexers>),
        ctx.today(),
        ctx.clock.now(),
    );
    // Whether each download client still files where lemonfiber wired it — the one field
    // an operator and lemonfiber both write, so the only place a fix could write over
    // somebody's own change. Read-only here: it says which side of the field moved, and
    // the repair it hands back refuses to move the operator's.
    let wiring = WiringCheck::new(
        ctx.http.clone(),
        ctx.filesystem.clone(),
        crate::app::seed::managed_wirings(ctx, &manifest.services, project.as_deref()).await,
        ctx.stamp(),
    );
    // Where the stack is actually listening, asked of the container engine rather
    // than read out of the files that asked for it: a mapping edited by hand and
    // applied, or an image whose defaults changed under an upgrade, is a service
    // answering somewhere nothing on disk says it does.
    let bindings = BindingsCheck::new(
        ctx.engine.clone(),
        ctx.settings.project.clone(),
        &manifest.services,
        ctx.environment,
    );
    let checks: Vec<Box<dyn Check>> = vec![
        Box::new(environment),
        Box::new(bindings),
        Box::new(storage),
        Box::new(vpn),
        Box::new(credentials),
        Box::new(indexer),
        Box::new(providers),
        Box::new(guides),
        Box::new(headroom),
        Box::new(releases),
        Box::new(wiring),
    ];
    Ok((manifest, checks))
}
