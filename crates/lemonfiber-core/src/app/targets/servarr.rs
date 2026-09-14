//! The Servarr-shape services whose credential can be proven, and the ones that
//! declared the shape and cannot be reached through it.
//!
//! Only a service that speaks the shape, publishes a port and names the file it writes
//! its key to can be a target. A service that declares no API at all is left out
//! silently and rightly: the manifest saying nothing is the stack's own statement that
//! there is nothing here to integrate with, which is a different thing from lemonfiber
//! being unable to.
//!
//! A service that declares the shape and is missing a part of it is neither, and used
//! to be treated as the first. It is an operator — on their own stack, which is the
//! only place this arises — who wrote an API declaration and got no feature working
//! against it, with nothing anywhere saying why. That case is named now.

use std::path::{Path, PathBuf};

use crate::doctor::credentials::Target;
use crate::model::UnsupportedReport;

use super::layout::config_path;

/// The Servarr-shape services whose credential can be proven, and where to read
/// each one's key and reach it.
///
/// Only a service that speaks the Servarr shape, publishes a port to reach it on
/// and names the config file it writes its key to can be proven; anything else is
/// left out rather than reported as a fault. The host path to that file follows
/// the stack's bind-mount convention — a service's `/config` is `config/<id>`
/// under the project root — so the key the service wrote is read from where
/// Compose mounted it.
pub(crate) fn servarr_targets(
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> Vec<Target> {
    let Some(project) = project else {
        return Vec::new();
    };
    services
        .iter()
        .filter_map(|service| target_for(service, project))
        .collect()
}

/// The one Servarr-shape service with this id, or nothing where the stack has none — the
/// lookup a reversal makes, which knows the name of the service it has to reach and
/// nothing else about it.
pub(crate) fn target_named(
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
    id: &str,
) -> Option<Target> {
    servarr_targets(services, project)
        .into_iter()
        .find(|target| target.id == id)
}

/// One service as a target to prove, or nothing where it cannot be one.
pub(crate) fn target_for(service: &lemonfiber_manifest::Service, project: &Path) -> Option<Target> {
    let api = service.api.as_ref()?;
    if api.kind != lemonfiber_manifest::ApiKind::Servarr {
        return None;
    }
    let port = service.port?;
    let config = config_path(project, service, api.path.as_deref())?;
    // The Servarr shape spans two API versions, so the manifest carries it. A
    // servarr service that names none cannot be reached at a known path, so it is
    // no target rather than one guessed at the wrong version.
    let version = api.version?;
    Some(Target {
        id: service.id.clone(),
        name: service.name.clone(),
        base: format!("http://127.0.0.1:{port}"),
        config,
        version,
    })
}

/// Every service that declares the Servarr shape and cannot be reached through it,
/// each with what its declaration is missing.
///
/// Nothing where the stack has not been written to disk: the config file half of the
/// question is answered against a project root, and with none of them the answer
/// would be "unreachable" for every service at once — which says something about the
/// machine rather than about any declaration.
pub(crate) fn unreachable_targets(
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> Vec<UnsupportedReport> {
    let Some(project) = project else {
        return Vec::new();
    };
    services
        .iter()
        .filter_map(|service| unreachable_for(service, project))
        .collect()
}

/// One service that declared the shape and cannot be reached through it, or nothing
/// where it can be — or where it never claimed to speak the shape at all.
fn unreachable_for(
    service: &lemonfiber_manifest::Service,
    project: &Path,
) -> Option<UnsupportedReport> {
    let api = service.api.as_ref()?;
    if api.kind != lemonfiber_manifest::ApiKind::Servarr {
        return None;
    }
    let because = missing_from(service, api, project)?;
    Some(UnsupportedReport {
        what: service.id.clone(),
        because,
    })
}

/// What a Servarr-shape declaration is missing, or nothing where it is complete.
///
/// In the order a reader would fix them, and one at a time: three sentences about one
/// service is a list nobody finishes, and the first of them is enough to act on.
fn missing_from(
    service: &lemonfiber_manifest::Service,
    api: &lemonfiber_manifest::Api,
    project: &Path,
) -> Option<String> {
    if service.port.is_none() {
        return Some(
            "it declares an API of the Servarr shape and no port to publish, so there is no \
             address to speak it on"
                .to_owned(),
        );
    }
    if api.version.is_none() {
        return Some(
            "the Servarr shape spans two API versions and this declaration names neither, so \
             the path to ask at would be a guess"
                .to_owned(),
        );
    }
    if config_path(project, service, api.path.as_deref()).is_none() {
        return Some(
            "it names no configuration file beneath a mount lemonfiber reads, so the key the \
             service writes for itself cannot be found"
                .to_owned(),
        );
    }
    None
}

/// Which download client a target is — and so which protocol its transfers move
/// over and which credential reaches it. `SABnzbd` carries the config file its key
/// is read from, so a resolved target always has one and the read never has to
/// check; qBittorrent carries nothing, reached with the recorded password instead.
pub(crate) enum DownloadKind {
    /// qBittorrent: torrents, reached with the recorded web UI password.
    Qbittorrent,
    /// `SABnzbd`: Usenet, reached with the key read from this config file.
    Sabnzbd {
        /// The config file `SABnzbd`'s key is read from.
        config: PathBuf,
    },
}

/// A download client the dashboard reads active transfers from: where to reach it
/// on the host, and which client it is.
pub(crate) struct DownloadTarget {
    /// Where to reach it on the host.
    pub base: String,
    /// Which client, so the caller picks the adapter, its credential and protocol.
    pub kind: DownloadKind,
}

#[cfg(test)]
mod tests {
    use super::{servarr_targets, unreachable_targets};
    use std::path::Path;

    /// One service, read back through the manifest parser rather than built as a
    /// struct, so what this test calls a declaration is what the stack's own file
    /// means by one.
    ///
    /// `api` is the whole of what varies, written out by the caller: the cases here
    /// are all about which part of a declaration is missing.
    /// A list of one rather than the service itself, and the assertion below is why:
    /// a parse that failed would otherwise need a way out of its own, and a block no
    /// run enters is a line the coverage gate counts against every honest one beside
    /// it. Said here rather than left to the cases, because most of them assert that
    /// nothing is reported — which an empty list satisfies while proving nothing.
    fn service(id: &str, port: Option<u16>, api: &str) -> Vec<lemonfiber_manifest::Service> {
        let published = port.map_or_else(String::new, |port| format!("port = {port}\n"));
        let written = format!(
            "schema_version = 1\nstack_version = \"0.1.0\"\nmin_cli_version = \"0.1.0\"\n\n\
             [[profile]]\nid = \"tv\"\nname = \"Television\"\ndescription = \"Television\"\n\n\
             [[service]]\nid = \"{id}\"\nname = \"{id}\"\nprofile = \"tv\"\n\
             image = \"example/{id}\"\ntag = \"1.0.0\"\n{published}\
             criticality = \"core\"\nlicense = \"GPL-3.0-only\"\n\
             upstream = \"https://example.invalid/{id}\"\nlast_release = \"2026-01-01\"\n\
             describes = \"Does a thing\"\nwithout_it = \"Do the thing yourself\"\n{api}"
        );
        let read: Vec<_> = lemonfiber_manifest::Manifest::from_toml(&written)
            .ok()
            .map(|manifest| manifest.services)
            .unwrap_or_default();
        assert_eq!(
            read.len(),
            1,
            "a manifest this test wrote is one the parser reads: {written}"
        );
        read
    }

    /// A complete Servarr declaration.
    const WHOLE: &str =
        "\n[service.api]\nkind = \"servarr\"\nkey_source = \"config-xml\"\npath = \"/config/config.xml\"\nversion = 3\n";

    /// Where the stack was written, which the config half of the question is asked
    /// against.
    fn project() -> &'static Path {
        Path::new("/somewhere/stack")
    }

    /// Why one service is unreachable, or nothing where it is not named at all.
    fn because(services: &[lemonfiber_manifest::Service]) -> Option<String> {
        unreachable_targets(services, Some(project()))
            .into_iter()
            .next()
            .map(|report| report.because)
    }

    #[test]
    fn a_complete_declaration_is_a_target_and_is_not_reported_as_anything_else() {
        let whole = service("theirs", Some(8989), WHOLE);
        assert_eq!(servarr_targets(&whole, Some(project())).len(), 1);
        assert_eq!(because(&whole), None);
    }

    /// The manifest saying nothing is the stack's own statement that there is nothing
    /// to integrate with, which is not lemonfiber failing to do something.
    #[test]
    fn a_service_declaring_no_api_at_all_is_not_reported_as_unsupported() {
        assert_eq!(because(&service("caddy", Some(80), "")), None);
    }

    /// Nor is a shape this file is not about. Another kind's own resolution answers
    /// for it, and two answers about one service is how they come to disagree.
    #[test]
    fn a_service_declaring_another_shape_is_left_to_whatever_answers_for_that_shape() {
        let other = "\n[service.api]\nkind = \"qbittorrent\"\nkey_source = \"generated\"\n";
        assert_eq!(because(&service("qbittorrent", Some(8080), other)), None);
    }

    #[test]
    fn a_declaration_with_no_port_is_named_with_nowhere_to_speak_to_it() {
        let said = because(&service("theirs", None, WHOLE)).unwrap_or_default();
        assert!(said.contains("no port"), "{said}");
    }

    #[test]
    fn a_declaration_naming_no_api_version_is_named_rather_than_guessed_at() {
        let versionless =
            "\n[service.api]\nkind = \"servarr\"\nkey_source = \"config-xml\"\npath = \"/config/config.xml\"\n";
        let said = because(&service("theirs", Some(8989), versionless)).unwrap_or_default();
        assert!(said.contains("two API versions"), "{said}");
    }

    #[test]
    fn a_declaration_whose_config_file_is_under_no_mount_lemonfiber_reads_is_named() {
        let elsewhere =
            "\n[service.api]\nkind = \"servarr\"\nkey_source = \"config-xml\"\npath = \"/etc/theirs.xml\"\nversion = 3\n";
        let said = because(&service("theirs", Some(8989), elsewhere)).unwrap_or_default();
        assert!(said.contains("configuration file"), "{said}");
    }

    /// The service is named, so an operator can go and find the declaration.
    #[test]
    fn what_is_unsupported_is_named_by_the_id_the_stack_declares_it_under() {
        let reports = unreachable_targets(&service("theirs", None, WHOLE), Some(project()));
        assert_eq!(
            reports.first().map(|report| report.what.clone()),
            Some("theirs".to_owned())
        );
    }

    /// With no stack on disk the config half cannot be asked, and answering
    /// "unreachable" for every service at once would say something about the machine
    /// rather than about any declaration.
    #[test]
    fn a_stack_that_has_not_been_written_yet_reports_nothing_unsupported() {
        assert!(unreachable_targets(&service("theirs", None, WHOLE), None).is_empty());
    }
}
