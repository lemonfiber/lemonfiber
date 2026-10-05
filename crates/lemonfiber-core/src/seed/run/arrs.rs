//! Seeding one media service.
//!
//! Everything a single \*arr needs pointed at it, and the order it has to happen in.

use super::clients::Held;
use super::connecting::{pairings, Connection};
use super::{
    category_for, escalate_broken_roots, skipped, target_for, wanted_roots, Ctx, Path, DATA_ROOT,
    SCHEMA_VERSION_FIELD,
};
use crate::ports::filesystem::Beneath;
use crate::ports::service::{Client, DownloadClient};
use crate::wiring::Fillers;

/// A Servarr application that files media: its identity and address (as the
/// credential check resolves them) and the media types it manages, which give
/// the root folders it needs.
pub(crate) struct Arr {
    pub(crate) target: crate::doctor::credentials::Target,
    pub(crate) media_types: Vec<String>,
}

/// The Servarr applications that file media — Sonarr, Radarr, Lidarr — resolved
/// with their media types. Prowlarr shares the shape but manages no media, so it
/// declares no media types and is left out.
pub(crate) fn servarr_arrs(
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

/// The inputs a seed pass reads once and hands to every \*arr it seeds: the
/// cross-\*arr contested-root map, who fills each ask and the credential each
/// download client answers to, the host data root each root folder is checked
/// against, the loaded baseline to compare with, and whether this is an adopt pass. Grouped so seeding one \*arr takes the pass and the
/// \*arr rather than a long list that only `arr` varies across.
pub(super) struct ArrSeeding<'a> {
    /// Root-folder paths more than one \*arr wants — refused rather than wired.
    pub(super) contested: &'a std::collections::BTreeMap<String, Vec<String>>,
    /// Who fills each of the stack's asks, and where each is reached.
    pub(super) fillers: &'a Fillers,
    /// The credential each download client answers to, where it is in hand.
    pub(super) held: &'a Held,
    /// The host directory `/data` resolves to, for the root-folder existence check.
    pub(super) data_root: Option<&'a Path>,
    /// What lemonfiber last recorded — the expected leg of the drift comparison.
    pub(super) expected: &'a crate::baseline::Baseline,
    /// Whether this pass adopts each drifted value as the accepted baseline.
    pub(super) adopt: bool,
}

/// Register an application's root folders, one per media type, under
/// `/data/media`, and its download clients beside them. The application's key is
/// read from its configuration; without it — the application has not finished
/// starting — both are skipped for a re-run rather than failed.
pub(super) async fn seed_arr(
    ctx: &Ctx,
    arr: &Arr,
    seeding: &ArrSeeding<'_>,
) -> (Vec<crate::seed::Wiring>, crate::baseline::Baseline) {
    let wanted = wanted_roots(&arr.media_types);
    let clients = wanted_clients(arr, seeding.fillers, seeding.held);
    // What this \*arr writes is recorded in its own baseline, against the loaded
    // snapshot, so several \*arrs can be seeded at once without sharing one; the
    // caller folds them back into one afterwards.
    let mut records = crate::baseline::Baseline::new();

    // The service's key is read once, opening the client for both its root folders
    // and its download clients rather than once each. Without it the service has not
    // finished starting, so both are skipped for a re-run and nothing is recorded.
    let Some(client) = arr
        .target
        .open(&ctx.seams.http, ctx.seams.filesystem.as_ref())
        .await
    else {
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
        crate::seed::Placing {
            contested: seeding.contested,
            root: DATA_ROOT,
        },
        &mut journal,
        &at,
        ctx.dry_run,
    )
    .await;
    // Before the download clients are appended, `wirings` holds exactly one entry per
    // wanted root folder in order, so each is escalated against the folder it reports
    // on: one the \*arr files into that resolves to nothing on the host is a root
    // folder pointing where nothing exists — a drift that breaks the stack.
    escalate_broken_roots(
        ctx.seams.filesystem.as_ref(),
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
                    rehearsing: ctx.dry_run,
                },
                &at,
            )
            .await,
        );
    }
    (wirings, records)
}

/// The download clients an \*arr is told about: each service filling one of its asks
/// that lemonfiber connects to it as a download client, at the address that service
/// declares, under the category the \*arr's first media type files as.
///
/// None where it manages no category. A filler whose credential is not in hand yet is
/// left out rather than told about with nothing to prove itself with, and a later run
/// that finds the credential tells the \*arr then.
pub(super) fn wanted_clients(arr: &Arr, fillers: &Fillers, held: &Held) -> Vec<DownloadClient> {
    let Some(category) = arr
        .media_types
        .first()
        .and_then(|media| category_for(media))
    else {
        return Vec::new();
    };
    let mut wanted = Vec::new();
    for pairing in pairings(fillers) {
        if pairing.ask.by != arr.target.id {
            continue;
        }
        let Ok((Connection::DownloadClient(kind), at, _)) = pairing.made else {
            continue;
        };
        let Some(credential) = held.of(&super::clients::Holder::of(pairing.filler)) else {
            continue;
        };
        wanted.push(DownloadClient {
            name: pairing.filler.name.clone(),
            host: at.host.clone(),
            port: at.port,
            kind,
            credential: credential.clone(),
            category: category.clone(),
        });
    }
    wanted
}

/// The API key a service of the Servarr shape wrote for itself, read from the file
/// its own declaration names — beneath its own directory where a plugin brought it.
///
/// [`Beneath::Read`] holds the key itself; a file holding none is as absent as one not
/// written, and a file refused stays refused, so the caller can say so.
pub(super) async fn servarr_key(ctx: &Ctx, filler: &crate::wiring::Filler) -> Beneath {
    match crate::app::targets::credential_file(ctx, filler).await {
        Beneath::Read(text) => {
            crate::servarr::api_key(&text).map_or(Beneath::Absent, Beneath::Read)
        }
        other => other,
    }
}

/// A connection refused because the filler's credential file was, saying why.
///
/// Refused rather than skipped: no later run reads the file while it stays what it is,
/// and it is either a mistake in the plugin or an attempt by it, which the operator has
/// to see either way.
pub(super) fn refused(connection: String, filler: &crate::wiring::Filler) -> crate::seed::Wiring {
    crate::seed::Wiring::settled(connection, refusal(filler))
}

/// What a connection comes to where the filler's credential file was refused.
pub(super) fn refusal(filler: &crate::wiring::Filler) -> crate::seed::State {
    crate::seed::State::Refused {
        reason: crate::app::targets::escaped(filler),
    }
}

/// A Servarr application's API key, read from the configuration file it wrote it
/// to, or nothing where it has not written one yet.
pub(super) async fn read_servarr_key(ctx: &Ctx, config: &Path) -> Option<String> {
    let within = crate::within::directory_of(config);
    let text =
        crate::app::targets::read_owned(ctx.seams.filesystem.as_ref(), config, within).await?;
    crate::servarr::api_key(&text)
}
