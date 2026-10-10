//! Putting a service's connections back to lemonfiber's own.
//!
//! The opposite of adopting: it discards an operator's edits, so it names every one of
//! them before it is confirmed and reverts nothing it did not show.

use super::{
    curators, load_baseline, project_directory, reading, save_baseline, wanted_clients, Ctx, Loaded,
};

/// Revert every drifted service connection to lemonfiber's own — or, unconfirmed, report
/// which would be. The connection side of a full reset: for each curator, a download-client
/// category the operator changed is written back through the update op (on confirm) or
/// only listed (on preview). Read-only until confirmed, so a preview changes nothing.
pub(crate) async fn reset_connections(ctx: &Ctx, confirm: bool) -> Vec<crate::seed::Wiring> {
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        return Vec::new();
    };
    // A register that is there and will not read is a machine that cannot say what fills
    // its asks, and a reset that went on would put back connections to a guess.
    let Ok(register) = crate::app::plugins::read(ctx) else {
        return Vec::new();
    };
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let (fillers, held) = reading(ctx, &manifest, register.installed(), project.as_deref()).await;
    let curating = curators(&fillers);
    let baseline = match load_baseline(ctx) {
        Loaded::Formed(baseline) => baseline,
        Loaded::Fresh | Loaded::Lost => crate::baseline::Baseline::new(),
    };
    let mut records = baseline.clone();
    let at = ctx.stamp();

    let mut wirings = Vec::new();
    for curator in curating {
        let wanted = wanted_clients(curator, &fillers, &held);
        if wanted.is_empty() {
            continue;
        }
        let Some(client) = curator.client(ctx).await else {
            continue;
        };
        wirings.extend(
            reset_curator_connections(
                client.as_ref(),
                curator.name(),
                &wanted,
                confirm,
                &baseline,
                &mut records,
                &at,
            )
            .await,
        );
    }
    // The aggregator's authentication, where lemonfiber turned it on and it has been
    // turned off since: a reset is what turns it back on.
    wirings.extend(super::guarding::put_back(ctx, &fillers, &baseline, confirm).await);
    if confirm {
        save_baseline(ctx, &records);
    }
    wirings
}

/// One curator's side of a connection reset: on confirm, revert each drifted
/// download-client category in place and report only the reverts that landed; on a
/// preview, read the clients and report which categories would be reverted, writing
/// nothing.
pub(super) async fn reset_curator_connections(
    client: &dyn crate::ports::service::Client,
    curator: &str,
    wanted: &[crate::ports::service::DownloadClient],
    confirm: bool,
    baseline: &crate::baseline::Baseline,
    records: &mut crate::baseline::Baseline,
    at: &str,
) -> Vec<crate::seed::Wiring> {
    if confirm {
        let mut journal = crate::journal::Journal::new();
        let reverted = crate::seed::wire_download_clients(
            client,
            curator,
            wanted,
            &mut journal,
            &mut crate::seed::Baselines {
                expected: baseline,
                records,
                adopt: false,
                reset: true,
                // A reset is never a rehearsal: an unconfirmed one previews through
                // `preview_reverts` above and never reaches the writing pass at all.
                rehearsing: false,
            },
            at,
        )
        .await;
        // A reset writes only its reverts, so a wired connection here is a drifted value
        // put back to lemonfiber's — the only outcome the report names.
        reverted
            .into_iter()
            .filter(|wiring| matches!(wiring.state, crate::seed::State::Wired))
            .collect()
    } else if let Ok(existing) = client.download_clients().await {
        preview_reverts(&existing, wanted, curator, baseline)
    } else {
        Vec::new()
    }
}

/// The connections a reset would revert, read only: each wanted client the service
/// holds whose category drifted from lemonfiber's. The same three-way comparison the
/// reverting pass makes, through the one shared observer, so a preview and a confirm
/// judge drift identically rather than by two hand-inlined comparisons.
pub(super) fn preview_reverts(
    existing: &[crate::ports::service::RegisteredClient],
    wanted: &[crate::ports::service::DownloadClient],
    curator: &str,
    baseline: &crate::baseline::Baseline,
) -> Vec<crate::seed::Wiring> {
    let mut wirings = Vec::new();
    for want in wanted {
        let Some(have) = existing
            .iter()
            .find(|have| have.host == want.host && have.port == want.port)
        else {
            continue;
        };
        let observed = crate::seed::observe_client(
            Some(have),
            want,
            baseline.entry(curator, &crate::seed::client_field(want)),
        );
        if matches!(
            observed,
            crate::seed::Observed::Drifted
                | crate::seed::Observed::Stale
                | crate::seed::Observed::Conflicted
                | crate::seed::Observed::Adopted
        ) {
            wirings.push(crate::seed::Wiring::settled(
                format!("{} into {curator}", want.name),
                crate::seed::State::Drifted,
            ));
        }
    }
    wirings
}
