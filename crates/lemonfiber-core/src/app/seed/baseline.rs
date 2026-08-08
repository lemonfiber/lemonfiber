//! The record of what lemonfiber last wrote.
//!
//! Without it a difference is only a difference; with it, an operator's edit can be
//! told from a default that moved. It is the only part of seeding that has to survive
//! between runs.

use super::{Ctx, Path};
use crate::config::store;

/// How the expected-state record read: genuinely absent, read, or there but
/// unreadable.
pub(super) enum Loaded {
    /// No record was found — a first seed, or none ever formed, or nothing
    /// configured. An empty baseline stands in and drift is assessed against it.
    Fresh,
    /// The record was read.
    Formed(crate::baseline::Baseline),
    /// The record was there but could not be read — its file could not be opened, or
    /// its contents did not parse. Drift cannot be assessed against it.
    Lost,
}

/// Read the expected-state baseline from where the last run left it, telling a
/// record that is genuinely absent from one that is there but lost.
///
/// It sits beside the environment file in the configuration directory; without that
/// path (nothing configured) there is nowhere to have kept one. A file that is not
/// there is a first seed; one that is there but cannot be opened or does not parse is
/// a loss — distinct, because the first is expected and the second must be reported.
///
/// Only a plain "not found" is read as a first seed. A read that fails any other way
/// — a directory in its place, a permission the run does not hold — is taken as a
/// loss rather than a first seed: the conservative direction, since a record that may
/// be there but unreadable is safer surfaced for the operator to re-baseline than
/// silently treated as one that never existed.
pub(super) fn load_baseline(ctx: &Ctx) -> Loaded {
    let Some(path) = baseline_path(ctx) else {
        return Loaded::Fresh;
    };
    match std::fs::read_to_string(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Loaded::Fresh,
        Err(_) => Loaded::Lost,
        Ok(text) => match serde_json::from_str(&text) {
            Ok(baseline) => Loaded::Formed(baseline),
            Err(_) => Loaded::Lost,
        },
    }
}

/// Write the baseline where the next run will read it. Best-effort, like the other
/// records seeding keeps: a run that cannot persist it still wired the stack, and
/// the worst a lost write costs is the next run re-reading current state as the
/// baseline rather than the prior one.
pub(super) fn save_baseline(ctx: &Ctx, baseline: &crate::baseline::Baseline) {
    if let Some(path) = baseline_path(ctx) {
        let _ = store::write(&path, &serde_json::to_string(baseline).unwrap_or_default());
    }
}

/// Where the baseline is kept: beside the environment file, in the configuration
/// directory, or nowhere when nothing is configured. Derived from the environment
/// file — the one path the context carries — and equal to
/// [`crate::config::paths::Paths::baseline`], which the layout keeps in the same
/// directory; the two must stay in step if that layout ever moves.
pub(super) fn baseline_path(ctx: &Ctx) -> Option<std::path::PathBuf> {
    crate::app::targets::beside_env(ctx, "baseline.json")
}

/// A timestamp for the change journal — seconds since the epoch, the clock's own
/// account of now.
pub(super) fn seed_stamp(ctx: &Ctx) -> String {
    ctx.clock
        .now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs().to_string())
        .unwrap_or_default()
}

/// The baseline field a service's own version is recorded under — a reserved key
/// that cannot collide with a download client's (keyed by endpoint) or any other
/// managed value, so the version rides in the same per-service record without a
/// separate store. Read on a later run to tell a service that upgraded its schema
/// from one still on the version lemonfiber last saw.
pub(super) const SCHEMA_VERSION_FIELD: &str = "schema:version";

/// Raise each wired root folder the host cannot back to a warning.
///
/// A root folder the \*arr files into must resolve to a directory on the host, where
/// the container's `/data` mount is rooted. One that resolves to nothing — the
/// operator repointed the data root, or the media directory was never created — is a
/// root folder pointing where nothing exists: the \*arr imports into a void. That
/// breaks the stack, so the folder's wiring is raised from information to a warning
/// naming the missing path and how to fix it.
///
/// Only folders the \*arr actually holds are checked — the wired and the
/// already-wired. A skipped or refused folder is not one the \*arr files into, so a
/// missing path there is not yet a break. Nothing is checked where no data root is
/// known, since without it the host path cannot be resolved to confirm or deny.
pub(super) async fn escalate_broken_roots(
    filesystem: &dyn crate::ports::filesystem::FileSystem,
    data_root: Option<&Path>,
    wanted: &[crate::ports::service::RootFolder],
    wirings: &mut [crate::seed::Wiring],
) {
    let Some(data_root) = data_root else {
        return;
    };
    for (folder, wiring) in wanted.iter().zip(wirings.iter_mut()) {
        if !matches!(
            wiring.state,
            crate::seed::State::Wired | crate::seed::State::AlreadyWired
        ) {
            continue;
        }
        // The host directory backing the container path — the `media/<type>` layout
        // `wanted_roots` builds, resolved against the operator's data root. A path that
        // does not resolve is one nothing is there to answer for.
        let host = data_root.join("media").join(&folder.media_type);
        if filesystem.canonicalize(&host).await.is_err() {
            wiring.escalate(
                format!(
                    "the root folder points where nothing exists: {}",
                    host.display()
                ),
                "create the directory, or point the folder at a path under the data root"
                    .to_owned(),
            );
        }
    }
}

/// Where the operator's data location is mounted inside every service: the tree a
/// root folder must sit within, so the service files where the downloads are
/// hardlinked and the rest of the stack can see them.
pub(super) const DATA_ROOT: &str = "/data";

/// The root folders an \*arr wants, one per media type it manages, each under the
/// media directory of the mounted data root. Shared by the seed pass and the
/// up-front contested-path check, so both reason about the same set.
pub(super) fn wanted_roots(media_types: &[String]) -> Vec<crate::ports::service::RootFolder> {
    media_types
        .iter()
        .map(|media| crate::ports::service::RootFolder {
            path: format!("{DATA_ROOT}/media/{media}"),
            media_type: media.clone(),
        })
        .collect()
}
