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

mod deferring;

use crate::doctor::autostart::AutostartCheck;
use crate::doctor::bindings::BindingsCheck;
use crate::doctor::credentials::CredentialsCheck;
use crate::doctor::declining::{Decline, DeclineKeyCheck};
use crate::doctor::environment::EnvironmentCheck;
use crate::doctor::gating::{Gate, GateRecordCheck};
use crate::doctor::guarded::GuardedCheck;
use crate::doctor::guides::GuidesCheck;
use crate::doctor::headroom::HeadroomCheck;
use crate::doctor::indexer::IndexerCheck;
use crate::doctor::releases::ReleasesCheck;
use crate::doctor::{examine, Check, Finding, Narrowing, Verdict};
use crate::error::{Diagnose, Problem, Remedy};
use crate::model::DoctorReport;

use crate::app::targets::{project_directory, servarr_targets};
use crate::app::Ctx;

use crate::error::codes::diag::NO_SUCH_CHECK;

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
    let (stack, checks) = assembled(ctx, disruptive).await?;
    let report = examined(ctx, &stack.manifest.services, &checks, narrowing).await;
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
    report.findings = crate::doctor::named(report.findings, services);
    // Last, so a finding already explained by another service's trouble is quoted with
    // its own output rather than instead of it. Reading a service's output is not the
    // check's own business, which is why it happens here and not inside one.
    report.findings = quoted(ctx, report.findings).await;
    report
}

/// How long the engine is given to say what the troubled services wrote lately.
///
/// The quoting comes after every check has answered, so nothing else bounds it, and
/// an engine that will not answer would otherwise hold a finished diagnosis back
/// without end. Past this the findings go out without their quotes.
const QUOTED_WITHIN: std::time::Duration = std::time::Duration::from_secs(10);

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

    let asked = super::settling::lately(ctx, &troubled);
    let lines = tokio::time::timeout(QUOTED_WITHIN, asked)
        .await
        .unwrap_or_default();
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

/// Whether the indexer the operator gave at setup still answers.
///
/// Re-proven the same way it was first proven — the shared validator over the same
/// HTTP seam — so a key that has since rotted is a finding rather than an empty
/// search weeks on.
fn indexer_still_answers(ctx: &Ctx) -> IndexerCheck {
    IndexerCheck::new(
        Arc::new(crate::validate::Allowed::new(
            Arc::new(crate::validate::Live::new(ctx.seams.http.clone())),
            ctx.settings.reaching.clone(),
        )),
        ctx.settings.indexer.clone(),
    )
}

/// What the request gate refused and removed since the last diagnosis, where the
/// stack runs one: its record, and the place last read kept beside the settings.
fn gate_record(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    project: Option<&std::path::Path>,
) -> GateRecordCheck {
    let gate = project
        .filter(|_| crate::app::gating::service(services).is_some())
        .map(|project| Gate {
            record: crate::app::gating::path(project, lemonfiber_sidecar::gate::File::Record),
            read: crate::app::targets::beside_env(ctx, crate::config::paths::GATE_READ),
        });
    GateRecordCheck::new(ctx.seams.filesystem.clone(), gate)
}

/// Whether the decline service's key was used for anything the service did not
/// record, where the stack runs one: its two records, and the media server that dates
/// the key.
fn decline_key(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    fillers: &crate::wiring::Fillers,
    project: Option<&std::path::Path>,
) -> DeclineKeyCheck {
    use lemonfiber_sidecar::decline::File;
    let decline = project
        .filter(|_| crate::app::invite::declining::service(services).is_some())
        .map(|project| Decline {
            refusals: crate::app::invite::declining::path(project, File::Refusals),
            lapses: crate::app::invite::declining::path(project, File::Lapses),
            keys: crate::app::targets::declined_reader(ctx, fillers)
                .map(|server| Arc::new(server) as Arc<dyn crate::doctor::declining::KeyDates>),
        });
    DeclineKeyCheck::new(ctx.seams.filesystem.clone(), decline)
}

/// Whether the stack's Usenet indexer aggregator keeps its configuration to itself, asked
/// of the one the stack ships where this machine can reach it.
fn guarded(ctx: &Ctx, services: &[lemonfiber_manifest::Service]) -> GuardedCheck {
    GuardedCheck::new(
        services
            .iter()
            .filter(|service| {
                service
                    .api
                    .as_ref()
                    .is_some_and(|api| api.kind == lemonfiber_manifest::ApiKind::Nzbhydra2)
            })
            .find_map(|service| {
                Some((
                    std::sync::Arc::new(crate::nzbhydra2::Nzbhydra2::new(
                        ctx.seams.http.clone(),
                        crate::app::targets::loopback(service.port?),
                        &service.id,
                    )) as std::sync::Arc<dyn crate::doctor::guarded::Exposure>,
                    service.name.clone(),
                ))
            }),
    )
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
) -> Result<(Stack, Vec<Box<dyn Check>>), Box<Problem>> {
    let stack = Stack {
        manifest: ctx
            .stack
            .checked_manifest(ctx.today())
            .map_err(|err| Box::new(err.problem()))?,
        // Refused rather than read past. A register that is there and will not parse is
        // a machine that cannot say what is installed on it, and a diagnosis that
        // carried on would report a clean bill of health with a stranger's rows silently
        // missing from it — which is the one answer a reader would act on and should not.
        installed: crate::app::plugins::read(ctx)?.installed().to_vec(),
    };
    let checks = assembling(ctx, &stack, disruptive);
    Ok((stack, checks))
}

/// Everything the checks are built out of, read once.
///
/// Two documents rather than one because they answer two questions and come from two
/// places: what this build ships is the stack's own manifest, and what an operator has
/// added to it is the register an install writes. A check list built from only the first
/// is a diagnosis that stops at the edge of what lemonfiber shipped.
pub(crate) struct Stack {
    /// What this build ships, and the services every bundled check is built against.
    pub manifest: lemonfiber_manifest::Manifest,
    /// What is installed on this machine, and the rows each of those added.
    pub installed: Vec<crate::plugin::Installed>,
}

/// The same list, built from a manifest somebody has already read.
///
/// Apart from the read above because reading the stack is the one thing here that can
/// refuse, and building the checks from what was read cannot. That matters to a caller
/// that has to look twice — an install holding the stack's verdict before its writes
/// against the same verdict after them — because the second look must be a fresh set of
/// checks, and a second look that could fail where the first did not would be a refusal
/// arriving after the machine had already been written to.
///
/// Fresh instances every time, and that is the point of asking again at all: a check
/// holds what it read when it was built, so re-running the same instances would compare
/// a machine against the very reading the work was meant to change.
pub(crate) fn assembling(ctx: &Ctx, stack: &Stack, disruptive: bool) -> Vec<Box<dyn Check>> {
    let manifest = &stack.manifest;
    let environment =
        EnvironmentCheck::reaching(ctx.seams.runner.clone(), ctx.settings.docker.clone());
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let storage = deferring::stored(ctx, manifest, project.as_deref());
    let vpn = deferring::tunnel(ctx, manifest, project.as_deref(), disruptive);
    let credentials = CredentialsCheck::new(
        ctx.seams.http.clone(),
        ctx.seams.filesystem.clone(),
        servarr_targets(&manifest.services, project.as_deref()),
    );
    let indexer = indexer_still_answers(ctx);
    // Whether the upstream quality guides can be reached, so a sync that would come
    // back empty — leaving the profiles in place stale rather than unconfigured — is
    // reported rather than silently missed.
    let guides = GuidesCheck::new(
        ctx.seams.http.clone(),
        ctx.settings
            .reaching
            .allows(crate::config::REACH_GUIDES_KEY),
    );
    // Whether the disk can plausibly hold a library at the chosen quality, projected
    // against the free space so an implausible choice is caught before it fills the
    // disk. The hungriest preset in force is the basis — the one that stresses the
    // disk most. An unreadable or unset choice falls back to the default rather than
    // failing the run.
    let headroom = HeadroomCheck::new(
        ctx.seams.filesystem.clone(),
        ctx.settings.data_root.clone(),
        crate::quality::run::most_demanding_or_default(ctx),
    );
    // Whether the chosen quality actually finds releases — a demanding preset can ask
    // for what the indexers do not carry, which reads as an indexer fault unless the two
    // are told apart. It searches for wanted content live, so it only does so on a
    // disruptive run; otherwise it reports skipped.
    let releases = ReleasesCheck::new(
        ctx.seams.http.clone(),
        ctx.seams.filesystem.clone(),
        servarr_targets(&manifest.services, project.as_deref()),
        disruptive,
    );
    let providers = deferring::providing(ctx, manifest, project.as_deref());
    let wiring = deferring::wired(ctx, stack, project.as_deref());
    let seasons = deferring::seasons(ctx, manifest);
    // Where the stack is actually listening, asked of the container engine rather
    // than read out of the files that asked for it: a mapping edited by hand and
    // applied, or an image whose defaults changed under an upgrade, is a service
    // answering somewhere nothing on disk says it does.
    let bindings = BindingsCheck::new(
        ctx.seams.engine.clone(),
        ctx.settings.project.clone(),
        &manifest.services,
        ctx.environment,
        &ctx.settings.exposed,
    );
    // Whether the files lemonfiber keeps credentials in are still readable only by
    // their owner. Read from the layout this machine resolved rather than from a
    // guessed path, and skipped where it resolved none — a check with nothing to look
    // at must not report that it looked.
    let permissions = crate::doctor::permissions::PermissionsCheck::new(
        ctx.seams.filesystem.clone(),
        crate::app::targets::layout(ctx)
            .as_ref()
            .map(crate::doctor::permissions::guarded)
            .unwrap_or_default(),
    );
    let telling = deferring::telling(ctx, manifest);
    let guarded = guarded(ctx, &manifest.services);
    let gate = gate_record(ctx, &manifest.services, project.as_deref());
    let decline = decline_key(
        ctx,
        &manifest.services,
        &crate::app::targets::fillers_here(ctx, manifest),
        project.as_deref(),
    );
    // Whether the stack would actually come back after a restart, which is a different
    // question from whether the operator asked for it to. The answer they gave is read
    // here rather than inside the check, for the reason every other reading is: a check
    // holds the seam it looks through, and lemonfiber's own records are not reached
    // through one.
    let autostart = AutostartCheck::new(
        ctx.seams.filesystem.clone(),
        ctx.seams.runner.clone(),
        ctx.environment,
        crate::autostart::run::load(ctx).wanted().on_boot(),
        ctx.settings.home.clone(),
    );
    let mut checks: Vec<Box<dyn Check>> = vec![
        Box::new(environment),
        Box::new(autostart),
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
        Box::new(seasons),
        Box::new(telling),
        Box::new(guarded),
        Box::new(gate),
        Box::new(decline),
        Box::new(permissions),
    ];
    // Appended to the same list rather than kept in one of their own, which is the
    // whole of what *run where the bundled ones are run* means: one register, one
    // budget, one narrowing, one overall. A second list beside this one would be a
    // second set of rules for a row an operator reads in the same report.
    //
    // Where each plugin's service answers is composed once, from the record of what was
    // installed, so a row asks the port the install published rather than a port
    // something guessed.
    let answering = crate::plugin::answering(&stack.installed);
    for installed in &stack.installed {
        checks.extend(crate::doctor::contributed::declared(
            installed,
            &answering,
            &ctx.seams.http,
        ));
    }
    checks
}

#[cfg(test)]
mod tests;
