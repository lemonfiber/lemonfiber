//! Where things sit on this machine.
//!
//! The project root a config is read under, the path a service's own config file takes
//! inside it, and the data root. One place spells the install layout, so a caller that
//! needs a path asks rather than rebuilding the convention.

use std::path::{Path, PathBuf};

use crate::app::Ctx;
use crate::config::paths::Paths;
use crate::config::store;
use crate::stack::Source;

/// The directory Compose treats as the project root, where the services' config
/// volumes are bind-mounted — the same path `up` hands Compose as
/// `--project-directory`, resolved here without writing anything.
///
/// An external stack is its own root; an embedded one lives wherever it was
/// materialised. Without that path there is nowhere to read a service's key from,
/// which the caller turns into no targets rather than a guess.
pub(crate) fn project_directory(stack: &Source, stack_dir: Option<&Path>) -> Option<PathBuf> {
    match stack {
        Source::External(path) => Some((*path).to_path_buf()),
        Source::Embedded(_) => stack_dir.map(Path::to_path_buf),
    }
}

/// The host path a service's config file is read from, per the stack's bind-mount
/// convention: a service's `/config` mount is `config/<id>` under the project root,
/// so its `api.path` of `/config/<inside>` is read from there. Nothing where the api
/// names no such path — the one place the convention is spelled, for every service
/// whose credential is read from disk.
pub(crate) fn config_path(
    project: &Path,
    service: &lemonfiber_manifest::Service,
    api_path: Option<&str>,
) -> Option<PathBuf> {
    let inside = api_path?.strip_prefix("/config/")?;
    Some(project.join("config").join(&service.id).join(inside))
}

/// A file kept beside the environment file — the one durable location the context
/// carries, so every record lemonfiber persists (the drift baseline, the materialised
/// checksums, the recorded quality choice) is placed the same way rather than each
/// re-deriving the directory. Nothing where no environment file is configured.
pub(crate) fn beside_env(ctx: &Ctx, name: &str) -> Option<PathBuf> {
    let env = ctx.settings.env_file.as_deref()?;
    Some(env.with_file_name(name))
}

/// The whole install layout, as the two locations a context already carries imply
/// it: the environment file sits directly in the configuration directory, and the
/// materialised stack directly in the data directory.
///
/// Nothing where either is unconfigured, which a caller turns into "there is
/// nowhere to keep this" rather than a guess. Resolved here beside the other two,
/// so a command that needs a whole layout asks for one instead of rebuilding the
/// convention out of the parts.
pub(crate) fn layout(ctx: &Ctx) -> Option<Paths> {
    let config = ctx.settings.env_file.as_deref()?.parent()?;
    let data = ctx.settings.stack_dir.as_deref()?.parent()?;
    Some(Paths::at(config, data))
}

/// The operator's data root on the host, as the environment file records it — the
/// directory the stack's `/data` mount resolves to. Read so a service's root folder,
/// a path inside that mount, can be checked against the filesystem it actually files
/// into: a folder resolving to nothing on the host is a root folder that breaks the
/// stack. Nothing where no environment file names a data root.
pub(crate) fn data_root(ctx: &Ctx) -> Option<std::path::PathBuf> {
    let path = ctx.settings.env_file.as_deref()?;
    let file = store::read(path).unwrap_or_default();
    crate::config::data_root_from_env(&file)
}
