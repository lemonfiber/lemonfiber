//! Putting a service's connections back to lemonfiber's own.
//!
//! The opposite of adopting: it discards an operator's edits, so it names every one of
//! them before it is confirmed and reverts nothing it did not show.

use super::{
    arr_download_clients, load_baseline, project_directory, read_sabnzbd_key, save_baseline,
    servarr_arrs, Ctx, Loaded,
};

/// Revert every drifted service connection to lemonfiber's own — or, unconfirmed, report
/// which would be. The connection side of a full reset: for each \*arr, a download-client
/// category the operator changed is written back through the update op (on confirm) or
/// only listed (on preview). Read-only until confirmed, so a preview changes nothing.
pub(crate) async fn reset_connections(ctx: &Ctx, confirm: bool) -> Vec<crate::seed::Wiring> {
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        return Vec::new();
    };
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let sabnzbd_key = read_sabnzbd_key(ctx, &manifest.services, project.as_deref()).await;
    let qbittorrent_password = crate::app::targets::recorded_qbittorrent_password(ctx);
    let arrs = servarr_arrs(&manifest.services, project.as_deref());
    let baseline = match load_baseline(ctx) {
        Loaded::Formed(baseline) => baseline,
        Loaded::Fresh | Loaded::Lost => crate::baseline::Baseline::new(),
    };
    let mut records = baseline.clone();
    let at = ctx.stamp();

    let mut wirings = Vec::new();
    for arr in &arrs {
        let wanted =
            arr_download_clients(arr, sabnzbd_key.as_deref(), qbittorrent_password.as_deref());
        if wanted.is_empty() {
            continue;
        }
        let Some(client) = arr.target.open(&ctx.http, ctx.filesystem.as_ref()).await else {
            continue;
        };
        wirings.extend(
            reset_arr_connections(
                &client,
                &arr.target.name,
                &wanted,
                confirm,
                &baseline,
                &mut records,
                &at,
            )
            .await,
        );
    }
    if confirm {
        save_baseline(ctx, &records);
    }
    wirings
}

/// One \*arr's side of a connection reset: on confirm, revert each drifted
/// download-client category in place and report only the reverts that landed; on a
/// preview, read the clients and report which categories would be reverted, writing
/// nothing.
pub(super) async fn reset_arr_connections(
    client: &dyn crate::ports::service::Client,
    arr_name: &str,
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
            arr_name,
            wanted,
            &mut journal,
            &mut crate::seed::Baselines {
                expected: baseline,
                records,
                adopt: false,
                reset: true,
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
        preview_reverts(&existing, wanted, arr_name, baseline)
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
    arr_name: &str,
    baseline: &crate::baseline::Baseline,
) -> Vec<crate::seed::Wiring> {
    let mut wirings = Vec::new();
    for want in wanted {
        let field = format!("downloadclient:{}:{}", want.host, want.port);
        let Some(have) = existing
            .iter()
            .find(|have| have.host == want.host && have.port == want.port)
        else {
            continue;
        };
        let observed =
            crate::seed::observe_client(Some(have), want, baseline.entry(arr_name, &field));
        if matches!(
            observed,
            crate::seed::Observed::Drifted
                | crate::seed::Observed::Stale
                | crate::seed::Observed::Conflicted
                | crate::seed::Observed::Adopted
        ) {
            wirings.push(crate::seed::Wiring::settled(
                format!("{} into {arr_name}", want.name),
                crate::seed::State::Drifted,
            ));
        }
    }
    wirings
}
