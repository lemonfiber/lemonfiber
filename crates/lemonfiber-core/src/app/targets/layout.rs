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

/// Where a service's configuration is mounted inside its own container.
///
/// `/config` for almost everything the stack runs, and `/app/config` for the request
/// service, which puts its own beneath its application directory. Tried in order, and
/// the longer one cannot be first: a path under `/config` never begins with the other,
/// but the reverse is not something to rely on.
const CONFIG_MOUNTS: [&str; 2] = ["/config/", "/app/config/"];

/// The host path a service's config file is read from, per the stack's bind-mount
/// convention: a service's config mount is `config/<id>` under the project root, so
/// its `api.path` of `<mount>/<inside>` is read from there. Nothing where the api
/// names no such path — the one place the convention is spelled, for every service
/// whose credential is read from disk.
///
/// The mount is not the same for every service, which is why it is a list rather than
/// one prefix. Reading a path that names a mount not here would silently resolve to
/// nothing, so a service whose credential cannot be found is worth checking against
/// this before anything else.
pub(crate) fn config_path(
    project: &Path,
    service: &lemonfiber_manifest::Service,
    api_path: Option<&str>,
) -> Option<PathBuf> {
    let path = api_path?;
    let inside = CONFIG_MOUNTS
        .iter()
        .find_map(|mount| path.strip_prefix(mount))?;
    Some(service_config_dir(project, &service.id).join(inside))
}

/// The directory on this machine one of the stack's own services owns: its configuration
/// directory, mounted into its container, and so a directory whatever runs there can
/// write anything into.
pub(crate) fn service_config_dir(project: &Path, id: &str) -> PathBuf {
    project.join(CONFIG_DIR).join(id)
}

/// A file one of the stack's services wrote, read only where it is a plain file beneath
/// `within`, the directory its container owns: nothing where it is not there yet, or
/// where something other than such a file is.
///
/// The container can write that directory, so a plain read would follow a link put where
/// the file is expected to any file on the host, or wait for ever on a pipe.
pub(crate) async fn read_owned(
    files: &dyn crate::ports::filesystem::FileSystem,
    path: &Path,
    within: &Path,
) -> Option<String> {
    files.read_beneath(path, within).await.text()
}

/// The directory a file of the stack is held beneath when it is read or written: its
/// service's configuration directory where it lies in one, since the container owns
/// everything inside that, and the project root otherwise.
pub(crate) fn held_beneath(project: &Path, relative: &Path) -> PathBuf {
    let mut parts = relative.components();
    match (parts.next(), parts.next(), parts.next()) {
        (Some(top), Some(service), Some(_)) if top.as_os_str() == CONFIG_DIR => {
            project.join(top).join(service)
        }
        _ => project.to_path_buf(),
    }
}

/// The host path an installed plugin's service has its credential read from.
///
/// The same convention as a bundled service's, with the one mount the plugin's own
/// record names in place of the list: lemonfiber mounts a plugin's configuration
/// directory wherever the plugin said it reads one, so a path beneath anywhere else is
/// a file nothing on this machine holds. Nothing where its adapter names no path.
///
/// **And nothing that could leave that directory.** The path is a stranger's, read back
/// from a record on disk, and what is read here is handed to the plugin's service as its
/// credential: a `..` in it, or a service id that is not one plain name, would have this
/// read any file on the host and send it away. The reader refuses such a manifest at
/// install, and this is the second wall rather than the first, because what is on disk is
/// not always what this build wrote.
///
/// A path is not the whole of it: the container owns the directory and can put a link
/// where the file is expected. What reads the file reads it through
/// [`crate::ports::filesystem::FileSystem::read_beneath`], held to
/// [`plugin_config_dir`].
pub(crate) fn plugin_config_path(
    project: &Path,
    placed: &crate::plugin::Placed,
) -> Option<PathBuf> {
    let path = placed.api.as_ref()?.path.as_deref()?;
    let inside = Path::new(
        path.strip_prefix(placed.config_path.as_str())?
            .strip_prefix('/')?,
    );
    let directory = plugin_config_dir(project, placed)?;
    plain(inside).then(|| directory.join(inside))
}

/// The directory on this machine an installed plugin's service owns: its configuration
/// directory, mounted into its container, and so a directory whatever runs there can
/// write anything into.
///
/// Nothing where the service's id is not one plain name, which would put the directory
/// somewhere other than beside every other service's.
pub(crate) fn plugin_config_dir(project: &Path, placed: &crate::plugin::Placed) -> Option<PathBuf> {
    let service = Path::new(&placed.service);
    (plain(service) && service.components().count() == 1)
        .then(|| project.join(CONFIG_DIR).join(service))
}

/// Whether a path is one or more plain names and nothing else: no root, no `..`, no `.`.
fn plain(path: &Path) -> bool {
    path.components().count() > 0
        && path
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
}

/// The directory under the project root each service's configuration directory sits in,
/// one per service and named for it.
const CONFIG_DIR: &str = "config";

/// The directory on this machine the stack's services keep their configuration and
/// databases beneath, one directory each.
pub(crate) fn services_config_dir(project: &Path) -> PathBuf {
    project.join(CONFIG_DIR)
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

#[cfg(test)]
mod tests;
