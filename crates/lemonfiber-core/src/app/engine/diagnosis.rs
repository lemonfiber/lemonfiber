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

use crate::doctor::credentials::CredentialsCheck;
use crate::doctor::environment::EnvironmentCheck;
use crate::doctor::guides::GuidesCheck;
use crate::doctor::headroom::HeadroomCheck;
use crate::doctor::indexer::IndexerCheck;
use crate::doctor::providers::ProvidersCheck;
use crate::doctor::releases::ReleasesCheck;
use crate::doctor::storage::StorageCheck;
use crate::doctor::vpn::VpnCheck;
use crate::doctor::{examine, Category, Check};
use crate::error::{Diagnose, Problem};
use crate::model::DoctorReport;
use crate::ports::service::{Indexers, UsenetAccounts};

use crate::app::targets::{committed_bytes, project_directory, servarr_targets};
use crate::app::Ctx;

/// Run the diagnostic checks, or the one category asked for.
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
/// the checks need before any of them can run.
pub async fn diagnose(
    ctx: &Ctx,
    only: Option<Category>,
    disruptive: bool,
) -> Result<DoctorReport, Problem> {
    let checks = assembled(ctx, disruptive).await?;
    Ok(examined(ctx, &checks, only).await)
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
    checks: &[Box<dyn Check>],
    only: Option<Category>,
) -> DoctorReport {
    let mut report = examine(checks, only).await;
    report.findings =
        crate::doctor::acknowledged::suppressing(report.findings, &crate::app::accepted::load(ctx));
    report
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
pub(crate) async fn assembled(ctx: &Ctx, disruptive: bool) -> Result<Vec<Box<dyn Check>>, Problem> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| err.problem())?;

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
    Ok(vec![
        Box::new(environment),
        Box::new(storage),
        Box::new(vpn),
        Box::new(credentials),
        Box::new(indexer),
        Box::new(providers),
        Box::new(guides),
        Box::new(headroom),
        Box::new(releases),
    ])
}
