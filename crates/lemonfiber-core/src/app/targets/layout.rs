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

#[cfg(test)]
mod tests {
    use super::config_path;

    fn a_service(id: &str) -> lemonfiber_manifest::Service {
        lemonfiber_manifest::Service {
            id: id.to_owned(),
            name: id.to_owned(),
            profile: "media".to_owned(),
            image: "example/image".to_owned(),
            tag: "1".to_owned(),
            port: None,
            bind: None,
            health: None,
            api: None,
            criticality: lemonfiber_manifest::Criticality::Core,
            license: "MIT".to_owned(),
            upstream: "https://example.test".to_owned(),
            last_release: "2026-01-01".to_owned(),
            describes: "an example service".to_owned(),
            without_it: "nothing works".to_owned(),
            media_types: Vec::new(),
            depends_on: Vec::new(),
            capabilities: Vec::new(),
            host_managed: false,
        }
    }

    /// A path under the usual mount reads from the service's own config directory.
    ///
    /// The nested case is the subtitle finder's, whose file sits a directory deeper —
    /// only the mount is stripped, not every `config` in the path.
    #[test]
    fn a_path_under_the_usual_mount_reads_from_the_services_own_directory() {
        let project = std::path::Path::new("/opt/lemonfiber/stack");
        assert_eq!(
            config_path(project, &a_service("sonarr"), Some("/config/config.xml")),
            Some(project.join("config/sonarr/config.xml"))
        );
        assert_eq!(
            config_path(
                project,
                &a_service("bazarr"),
                Some("/config/config/config.yaml")
            ),
            Some(project.join("config/bazarr/config/config.yaml"))
        );
    }

    /// The request service mounts its config beneath its application directory.
    ///
    /// Not every service uses the same mount, and a path naming one this does not know
    /// resolves to nothing rather than to a wrong file — which is why the list of
    /// mounts is the first thing to check when a credential cannot be found.
    #[test]
    fn a_path_under_the_application_mount_reads_from_the_same_place() {
        let project = std::path::Path::new("/opt/lemonfiber/stack");
        assert_eq!(
            config_path(
                project,
                &a_service("seerr"),
                Some("/app/config/settings.json")
            ),
            Some(project.join("config/seerr/settings.json"))
        );
    }

    /// A path naming no mount this knows, and no path at all, both resolve to nothing.
    #[test]
    fn a_path_outside_every_known_mount_resolves_to_nothing() {
        let project = std::path::Path::new("/opt/lemonfiber/stack");
        assert_eq!(
            config_path(project, &a_service("odd"), Some("/etc/odd/settings.json")),
            None
        );
        assert_eq!(config_path(project, &a_service("odd"), None), None);
    }
}
