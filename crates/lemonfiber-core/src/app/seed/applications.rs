//! Pushing the services at the indexer that searches for them.
//!
//! The indexer needs to know what to search on behalf of, which is the one connection
//! that runs from the indexer outward rather than into it.

use super::{read_servarr_key, target_for, Ctx, Path, PathBuf};

/// Prowlarr as the app-sync source, and the media-filing \*arrs to register into
/// it — the resolution app sync starts from.
pub(super) struct AppSyncSource {
    /// Prowlarr itself: where to reach its API and read its key.
    pub(super) target: crate::doctor::credentials::Target,
    /// The address the \*arrs reach Prowlarr back on, on the stack's network.
    pub(super) network_url: String,
}

/// One media-filing \*arr to register into Prowlarr: what to call it, which
/// application it is, where to read its key, and where Prowlarr reaches it.
pub(super) struct SyncableArr {
    pub(super) name: String,
    pub(super) kind: crate::ports::service::ApplicationKind,
    pub(super) config: PathBuf,
    pub(super) network_url: String,
}

/// Register each media-filing \*arr as an application in Prowlarr, so Prowlarr
/// pushes it indexers.
///
/// Two keys gate a write, both read from configuration and never asked for:
/// Prowlarr's own — without it Prowlarr is still starting, so every application
/// is skipped for a re-run — and each \*arr's, which is what lets Prowlarr write
/// into that \*arr; an \*arr that has not written its key yet is skipped on its
/// own while the others proceed.
pub(super) async fn seed_applications(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> Vec<crate::seed::Wiring> {
    let Some(source) = prowlarr_source(services, project) else {
        return Vec::new();
    };

    let Some(prowlarr_key) = read_servarr_key(ctx, &source.target.config).await else {
        return syncable_arrs(services, project)
            .into_iter()
            .map(|arr| skipped_application(&arr.name, &source.target.name))
            .collect();
    };

    let mut wanted = Vec::new();
    let mut skipped = Vec::new();
    for arr in syncable_arrs(services, project) {
        match read_servarr_key(ctx, &arr.config).await {
            Some(key) => wanted.push(crate::ports::service::Application {
                name: arr.name,
                kind: arr.kind,
                prowlarr_url: source.network_url.clone(),
                base_url: arr.network_url,
                api_key: key,
            }),
            None => skipped.push(skipped_application(&arr.name, &source.target.name)),
        }
    }

    let client = crate::prowlarr::Prowlarr::new(
        ctx.http.clone(),
        &source.target.base,
        prowlarr_key,
        &source.target.id,
    );
    // The journal seed records each write into is not persisted: seeding is
    // idempotent, so a partial run is recovered by running it again, not reversed
    // — see the seed module doc. The record is groundwork for a future service-side
    // undo the current reversal cannot do.
    let mut journal = crate::journal::Journal::new();
    let mut wirings = crate::seed::wire_applications(
        &client,
        &source.target.name,
        &wanted,
        &mut journal,
        &ctx.stamp(),
    )
    .await;
    wirings.extend(skipped);
    wirings
}

/// Prowlarr as the app-sync source: the Servarr-shape service that manages no
/// media, reached at its published port and known on the network by its own
/// container name. Nothing where the stack has no such service or no project to
/// read its key from.
pub(super) fn prowlarr_source(
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> Option<AppSyncSource> {
    // The target's api version is required for it to resolve, but Prowlarr's own
    // client fixes v1, so it is the config path here that is load-bearing, not the
    // version the target carries.
    let target = super::super::targets::aggregator_target(services, project)?;
    let port = services
        .iter()
        .find(|service| service.id == target.id)
        .and_then(|service| service.port)?;
    Some(AppSyncSource {
        network_url: format!("http://{}:{port}", target.id),
        target,
    })
}

/// The media-filing \*arrs Prowlarr's app sync covers, each as the application it
/// is and where Prowlarr reaches it on the network. A Servarr service whose media
/// is not one app sync covers — or none — is left out rather than guessed at.
pub(super) fn syncable_arrs(
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> Vec<SyncableArr> {
    let Some(project) = project else {
        return Vec::new();
    };
    services
        .iter()
        .filter_map(|service| {
            let target = target_for(service, project)?;
            let kind = application_kind(&service.media_types)?;
            let port = service.port?;
            Some(SyncableArr {
                name: service.name.clone(),
                kind,
                config: target.config,
                network_url: format!("http://{}:{port}", service.id),
            })
        })
        .collect()
}

/// The Prowlarr application kind for an \*arr's media, or nothing where its media
/// is not one Prowlarr's app sync covers — the same by-media mapping the download
/// clients' categories use, so the two stay in step.
pub(super) fn application_kind(
    media_types: &[String],
) -> Option<crate::ports::service::ApplicationKind> {
    match media_types.first().map(String::as_str) {
        Some("tv") => Some(crate::ports::service::ApplicationKind::Sonarr),
        Some("movies") => Some(crate::ports::service::ApplicationKind::Radarr),
        Some("music") => Some(crate::ports::service::ApplicationKind::Lidarr),
        _ => None,
    }
}

/// A `Wiring` skipped because the service has not written the key it needs yet — a
/// re-run completes it. The reason is worded once here so a re-run's report reads
/// the same across root folders, download clients and application sync; the
/// `connection` names the specific edge being skipped.
pub(super) fn skipped(connection: String, service: &str) -> crate::seed::Wiring {
    crate::seed::Wiring::settled(
        connection,
        crate::seed::State::Skipped {
            reason: format!("{service} has not written its API key yet; a later run completes it"),
        },
    )
}

/// An application skipped for a re-run because Prowlarr's key is not written yet.
pub(super) fn skipped_application(arr: &str, prowlarr: &str) -> crate::seed::Wiring {
    skipped(format!("{arr} indexer sync via {prowlarr}"), arr)
}
