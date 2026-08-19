//! Seeding one media service.
//!
//! Everything a single \*arr needs pointed at it, and the order it has to happen in.

use super::{
    category_for, download_clients, escalate_broken_roots, skipped, target_for, wanted_roots, Ctx,
    Path, DATA_ROOT, SCHEMA_VERSION_FIELD,
};
use crate::ports::service::Client;

/// A Servarr application that files media: its identity and address (as the
/// credential check resolves them) and the media types it manages, which give
/// the root folders it needs.
pub(super) struct Arr {
    pub(super) target: crate::doctor::credentials::Target,
    pub(super) media_types: Vec<String>,
}

/// The Servarr applications that file media — Sonarr, Radarr, Lidarr — resolved
/// with their media types. Prowlarr shares the shape but manages no media, so it
/// declares no media types and is left out.
pub(super) fn servarr_arrs(
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> Vec<Arr> {
    let Some(project) = project else {
        return Vec::new();
    };
    services
        .iter()
        .filter_map(|service| {
            let target = target_for(service, project)?;
            if service.media_types.is_empty() {
                return None;
            }
            Some(Arr {
                target,
                media_types: service.media_types.clone(),
            })
        })
        .collect()
}

/// Register an application's root folders, one per media type, under
/// `/data/media`. The application's key is read from its configuration; without
/// it — the application has not finished starting — the folders are skipped for a
/// re-run rather than failed.
/// The inputs a seed pass reads once and hands to every \*arr it seeds: the
/// cross-\*arr contested-root map, the download-client credentials, the host data
/// root each root folder is checked against, the loaded baseline to compare with, and
/// whether this is an adopt pass. Grouped so seeding one \*arr takes the pass and the
/// \*arr rather than a long list that only `arr` varies across.
pub(super) struct ArrSeeding<'a> {
    /// Root-folder paths more than one \*arr wants — refused rather than wired.
    pub(super) contested: &'a std::collections::BTreeMap<String, Vec<String>>,
    /// `SABnzbd`'s API key, where it has written one.
    pub(super) sabnzbd_key: Option<&'a str>,
    /// qBittorrent's web UI password, minted this run or recorded on an earlier one.
    pub(super) qbittorrent_password: Option<&'a str>,
    /// The host directory `/data` resolves to, for the root-folder existence check.
    pub(super) data_root: Option<&'a Path>,
    /// What lemonfiber last recorded — the expected leg of the drift comparison.
    pub(super) expected: &'a crate::baseline::Baseline,
    /// Whether this pass adopts each drifted value as the accepted baseline.
    pub(super) adopt: bool,
}

pub(super) async fn seed_arr(
    ctx: &Ctx,
    arr: &Arr,
    seeding: &ArrSeeding<'_>,
) -> (Vec<crate::seed::Wiring>, crate::baseline::Baseline) {
    let wanted = wanted_roots(&arr.media_types);
    let clients = arr_download_clients(arr, seeding.sabnzbd_key, seeding.qbittorrent_password);
    // What this \*arr writes is recorded in its own baseline, against the loaded
    // snapshot, so several \*arrs can be seeded at once without sharing one; the
    // caller folds them back into one afterwards.
    let mut records = crate::baseline::Baseline::new();

    // The service's key is read once, opening the client for both its root folders
    // and its download clients rather than once each. Without it the service has not
    // finished starting, so both are skipped for a re-run and nothing is recorded.
    let Some(client) = arr.target.open(&ctx.http, ctx.filesystem.as_ref()).await else {
        let mut wirings: Vec<_> = wanted
            .iter()
            .map(|folder| {
                skipped(
                    format!("{} root folder in {}", folder.media_type, arr.target.name),
                    &arr.target.name,
                )
            })
            .collect();
        wirings.extend(clients.iter().map(|client| {
            skipped(
                format!("{} into {}", client.name, arr.target.name),
                &arr.target.name,
            )
        }));
        return (wirings, records);
    };

    // The journal seed records each write into is not persisted: seeding is
    // idempotent, so a partial run is recovered by running it again, not reversed
    // — see the seed module doc.
    let at = ctx.stamp();
    let mut journal = crate::journal::Journal::new();

    // A schema change re-baselines rather than reporting mass drift. The service's own
    // version, read from its status, is compared with the one lemonfiber last recorded:
    // a change, taken with every download client drifted at once, is a service that
    // renamed its fields on upgrade — not the operator hand-editing each — so the
    // current shape is adopted as the new baseline instead of every field reported as
    // drift. A version change alone, with only some fields drifted, is left as the
    // genuine operator edits it is. The live version is recorded either way, so the
    // next run compares against what the service is now on.
    let live_version = client
        .identity()
        .await
        .ok()
        .map(|identity| identity.version);
    let version_changed = match (
        &live_version,
        seeding
            .expected
            .expected(&arr.target.name, SCHEMA_VERSION_FIELD),
    ) {
        (Some(live), Some(recorded)) => live != recorded,
        _ => false,
    };
    let re_baseline = version_changed
        && !clients.is_empty()
        && client.download_clients().await.is_ok_and(|existing| {
            crate::seed::wholesale_drift(&existing, &clients, seeding.expected, &arr.target.name)
        });
    if let Some(live) = &live_version {
        records.record(&arr.target.name, SCHEMA_VERSION_FIELD, live, &at);
    }

    let mut wirings = crate::seed::wire_root_folders(
        &client,
        &arr.target.name,
        &wanted,
        seeding.contested,
        DATA_ROOT,
        &mut journal,
        &at,
    )
    .await;
    // Before the download clients are appended, `wirings` holds exactly one entry per
    // wanted root folder in order, so each is escalated against the folder it reports
    // on: one the \*arr files into that resolves to nothing on the host is a root
    // folder pointing where nothing exists — a drift that breaks the stack.
    escalate_broken_roots(
        ctx.filesystem.as_ref(),
        seeding.data_root,
        &wanted,
        &mut wirings,
    )
    .await;
    if !clients.is_empty() {
        wirings.extend(
            crate::seed::wire_download_clients(
                &client,
                &arr.target.name,
                &clients,
                &mut journal,
                &mut crate::seed::Baselines {
                    expected: seeding.expected,
                    records: &mut records,
                    adopt: seeding.adopt || re_baseline,
                    reset: false,
                },
                &at,
            )
            .await,
        );
    }
    (wirings, records)
}

/// The download clients an \*arr registers — one per credential in hand — under the
/// category its first media type files as, or none where it manages no category.
pub(super) fn arr_download_clients(
    arr: &Arr,
    sabnzbd_key: Option<&str>,
    qbittorrent_password: Option<&str>,
) -> Vec<crate::ports::service::DownloadClient> {
    arr.media_types
        .first()
        .and_then(|media| category_for(media))
        .map(|category| download_clients(sabnzbd_key, qbittorrent_password, &category))
        .unwrap_or_default()
}

/// A Servarr application's API key, read from the configuration file it wrote it
/// to, or nothing where it has not written one yet.
pub(super) async fn read_servarr_key(ctx: &Ctx, config: &Path) -> Option<String> {
    let text = ctx.filesystem.read(config).await?;
    crate::servarr::api_key(&text)
}
