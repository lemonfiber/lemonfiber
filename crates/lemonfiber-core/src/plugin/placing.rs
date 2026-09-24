//! Where an install puts what it writes, and what each of those writes is.
//!
//! The install record settles *what* was decided; this settles *where that lands on
//! the machine*. Kept apart from both the record and the container for the reason
//! the container is kept apart from the record: a copy of a derivation is free to
//! disagree with the derivation, so there is one place that turns an installed
//! plugin into a list of paths and one place that turns it into a container.
//!
//! **Almost every write is a path lemonfiber creates, and that is what makes the
//! reversal ordinary.** A file the install makes is journalled as
//! [`crate::journal::Kind::Made`], which the rollback layer already classifies as
//! reversible in full and already undoes by removing exactly the path that was made.
//!
//! **One file per plugin rather than one file for all of them.** A single shared
//! document would be rewritten by every install and every removal, which makes each
//! of those a change to a file that already existed — and the honest journal entry
//! for that is not `Made`. Per plugin, an install creates exactly one document and a
//! removal removes exactly the one it created, so what the record says and what the
//! reversal does are the same sentence.
//!
//! **The exception is the stack's own wiring.** The proxy reads one file and the
//! dashboard another, and a plugin's service is reachable through the first and
//! listed on the second only if it is written *into* them. So those two writes are a
//! region each, marked out inside the file and owned by the plugin
//! ([`crate::region`]), and journalled as [`crate::journal::Kind::Region`]: the
//! reversal takes out exactly the region, and refuses where somebody has edited it.
//! What goes in them is [`super::fronting`]'s to say.
//!
//! Nothing here touches a disk. It is given a record and a stack directory and
//! answers with a list, so every arrangement can be put in front of a test without
//! one.

use std::path::{Path, PathBuf};

use super::installed::Installed;

/// The directory beneath the stack where a plugin's Compose document is written.
///
/// Inside the stack directory because Compose resolves `extends.file` against the
/// document that declares it, and the entry lemonfiber writes extends the stack's
/// own template by a relative path. A document kept anywhere else would name a
/// template that is not there.
const OVERLAYS: &str = "compose/plugins";

/// The directory beneath the stack holding each service's own configuration.
///
/// The same one the bundled services use, and the same one the generated container
/// mounts from — `./config/<service>` in the entry is this directory, resolved
/// against the project directory Compose is pointed at.
const CONFIGURATION: &str = "config";

/// One thing an install puts on the machine.
///
/// A path and what lands at it. The two are one type rather than two because the
/// caller does the same thing with every one — journals it, then makes it — and a
/// caller holding two lists is a caller free to journal one and write the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Write {
    /// Where it goes.
    pub path: PathBuf,
    /// What lands there.
    pub lands: Lands,
}

/// What one write puts at its path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lands {
    /// A directory brought into being.
    Directory,
    /// A file written whole, holding this.
    Document(String),
    /// A region inside a file the stack already has, holding this.
    Region {
        /// The file beneath the stack directory, as the record of what lemonfiber
        /// materialised names it.
        key: String,
        /// Whose region it is, as its markers name it.
        owner: String,
        /// What it holds.
        body: String,
    },
}

impl Write {
    /// A directory the install makes.
    const fn directory(path: PathBuf) -> Self {
        Self {
            path,
            lands: Lands::Directory,
        }
    }

    /// A file the install writes, and what it holds.
    const fn file(path: PathBuf, content: String) -> Self {
        Self {
            path,
            lands: Lands::Document(content),
        }
    }

    /// A region the install writes into one of the stack's own files.
    fn region(stack: &Path, key: &str, owner: String, body: String) -> Self {
        Self {
            path: stack.join(key),
            lands: Lands::Region {
                key: key.to_owned(),
                owner,
                body,
            },
        }
    }

    /// Whether this write is a directory rather than a file.
    #[must_use]
    pub const fn is_directory(&self) -> bool {
        matches!(self.lands, Lands::Directory)
    }
}

/// Where a plugin's Compose document is written, beneath the stack directory.
///
/// Public because the settings have to layer it on every invocation, and a caller
/// that rebuilt the path itself would be a second answer to where it is.
#[must_use]
pub fn overlay(stack: &Path, plugin: &str) -> PathBuf {
    stack.join(OVERLAYS).join(format!("{plugin}.yml"))
}

/// Where one of a plugin's services keeps its own configuration.
///
/// Not published, unlike the document's path: nothing outside this module has had to
/// ask yet, and a name nothing reads is one that drifts from what it describes.
fn configuration(stack: &Path, service: &str) -> PathBuf {
    stack.join(CONFIGURATION).join(service)
}

/// Every Compose document the installed plugins contribute, in the order Compose
/// reads them.
///
/// Read off the register rather than by listing the directory the documents sit in.
/// A listing would run whatever it found there, which turns a file somebody dropped
/// in by hand into a service in the stack — and the register is the only thing that
/// says what this machine agreed to install.
///
/// A plugin whose document has gone missing is named anyway rather than skipped.
/// Compose refuses a file it cannot read, which is the honest outcome: the register
/// says the plugin is installed, and a run that quietly started the stack without it
/// would be answering for a stack the operator does not have.
///
/// Named by id and joined here rather than carried as resolved paths, because where
/// a document lives depends on which directory this invocation calls the project
/// root — and a path settled anywhere else would be right for the embedded stack and
/// quietly wrong for one the operator named.
///
/// In the register's own order, which is the plugins' ids — so the invocation is the
/// same twice and a cached Compose project does not churn for no reason.
#[must_use]
pub fn documents(installed: &[String], stack: &Path) -> Vec<PathBuf> {
    installed
        .iter()
        .map(|plugin| overlay(stack, plugin))
        .collect()
}

/// Everything installing this plugin writes, in the order it is written.
///
/// Ordered, and the order is load-bearing twice over. The configuration directories
/// come first because the Compose document mounts them, so a stop between the two
/// leaves directories nothing references rather than an entry mounting a directory
/// that is not there. And within the directories a parent comes before its child, so
/// a reversal walking the record backwards removes the child first.
///
/// The document comes after them for the same reason the applied marker is last in an
/// apply: it is the write that makes the rest take effect, so a run that stopped short
/// of it has changed nothing Compose will read. The proxy's region and the dashboard's
/// follow it, because a route to a service that is not declared is a route to nothing.
/// Each is planned only where it would hold something, so a plugin whose services
/// nothing reaches writes into neither.
#[must_use]
pub fn writes(installed: &Installed, stack: &Path) -> Vec<Write> {
    let mut planned: Vec<Write> = installed
        .services
        .iter()
        .map(|placed| Write::directory(configuration(stack, &placed.service)))
        .collect();
    planned.push(Write::file(
        overlay(stack, &installed.plugin),
        super::container::written(installed),
    ));
    let owner = super::fronting::owner(&installed.plugin);
    for (key, body) in [
        (super::fronting::PROXY, super::fronting::proxied(installed)),
        (
            super::fronting::DASHBOARD,
            super::fronting::listed(installed),
        ),
    ] {
        if !body.is_empty() {
            planned.push(Write::region(stack, key, owner.clone(), body));
        }
    }
    planned
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{configuration, documents, overlay, writes, Lands, Write};
    use crate::plugin::installed::{Installed, Placed, Reached};

    /// The stack directory these are written beneath.
    fn stack() -> &'static Path {
        Path::new("/opt/lemonfiber/stack")
    }

    /// One placed service, as a record holds one.
    fn placed(service: &str) -> Placed {
        Placed {
            service: service.to_owned(),
            image: "example.invalid/komga".to_owned(),
            digest: "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
                .to_owned(),
            tag: "1.11.0".to_owned(),
            config_path: "/app/data".to_owned(),
            takes_data: true,
            reached: Some(Reached::Household {
                port: 25600,
                hostname: "komga".to_owned(),
                group: None,
            }),
            provides: Vec::new(),
            name: "Komga".to_owned(),
            description: "Reads comics".to_owned(),
        }
    }

    /// An installed plugin holding the named services.
    fn installed(plugin: &str, services: &[&str]) -> Installed {
        Installed {
            plugin: plugin.to_owned(),
            version: "1.2.0".to_owned(),
            services: services.iter().map(|one| placed(one)).collect(),
            provides: Vec::new(),
            contributions: Vec::new(),
            declared: crate::plugin::Declaration::default(),
            from: String::new(),
            installed_at: String::new(),
        }
    }

    #[test]
    fn a_plugin_s_document_is_written_beneath_the_stack_where_compose_reads_it() {
        assert_eq!(
            overlay(stack(), "komga"),
            PathBuf::from("/opt/lemonfiber/stack/compose/plugins/komga.yml")
        );
    }

    /// The document extends the stack's own template by a relative path, so it has to
    /// sit where that path resolves. A document written outside the stack would name a
    /// template that is not there, and Compose would refuse the whole project — every
    /// bundled service with it.
    #[test]
    fn the_document_sits_where_the_template_it_extends_resolves() {
        let at = overlay(stack(), "komga");
        let template = at
            .parent()
            .and_then(Path::parent)
            .map(|inner| inner.join("_common.yml"));
        assert_eq!(
            template,
            Some(PathBuf::from("/opt/lemonfiber/stack/compose/_common.yml"))
        );
    }

    #[test]
    fn a_service_keeps_its_configuration_where_the_bundled_ones_keep_theirs() {
        assert_eq!(
            configuration(stack(), "komga"),
            PathBuf::from("/opt/lemonfiber/stack/config/komga")
        );
    }

    /// What the one document in a plan holds.
    fn document(planned: &[Write]) -> Option<String> {
        planned.iter().find_map(|one| match &one.lands {
            Lands::Document(content) => Some(content.clone()),
            Lands::Directory | Lands::Region { .. } => None,
        })
    }

    /// Every region a plan writes, as the file it goes in and whose it is.
    fn regions(planned: &[Write]) -> Vec<(String, String)> {
        planned
            .iter()
            .filter_map(|one| match &one.lands {
                Lands::Region { key, owner, .. } => Some((key.clone(), owner.clone())),
                Lands::Directory | Lands::Document(_) => None,
            })
            .collect()
    }

    /// A household service gets a region in the proxy and one on the dashboard, both
    /// the plugin's, both after the document that declares the service.
    #[test]
    fn a_household_service_is_written_into_the_proxy_and_the_dashboard_after_its_document() {
        let planned = writes(&installed("komga", &["komga"]), stack());

        assert_eq!(
            regions(&planned),
            vec![
                (crate::plugin::PROXY.to_owned(), "plugin komga".to_owned()),
                (
                    crate::plugin::DASHBOARD.to_owned(),
                    "plugin komga".to_owned()
                ),
            ]
        );
        assert!(planned
            .iter()
            .take(planned.len() - 2)
            .all(|one| !matches!(one.lands, Lands::Region { .. })));
    }

    /// The tier decides: an operator surface is listed and not proxied, and a service
    /// nothing reaches is neither, so a plugin of only those writes no proxy region.
    #[test]
    fn a_service_the_household_does_not_reach_is_given_no_route() {
        let mut plugin = installed("komga", &["komga", "worker"]);
        let _ = plugin.services.first_mut().map(|one| {
            one.reached = Some(crate::plugin::Reached::Loopback {
                port: 8090,
                group: None,
            });
        });
        let _ = plugin.services.get_mut(1).map(|one| one.reached = None);

        assert_eq!(
            regions(&writes(&plugin, stack())),
            vec![(
                crate::plugin::DASHBOARD.to_owned(),
                "plugin komga".to_owned()
            )]
        );
    }

    #[test]
    fn a_plugin_whose_services_nothing_reaches_writes_into_neither() {
        let mut plugin = installed("komga", &["komga"]);
        let _ = plugin.services.first_mut().map(|one| one.reached = None);

        assert!(regions(&writes(&plugin, stack())).is_empty());
    }

    #[test]
    fn installing_writes_a_directory_for_each_service_and_one_document_for_the_plugin() {
        let planned = writes(&installed("komga", &["komga", "komga-worker"]), stack());
        let paths: Vec<PathBuf> = planned.iter().map(|one| one.path.clone()).collect();
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/opt/lemonfiber/stack/config/komga"),
                PathBuf::from("/opt/lemonfiber/stack/config/komga-worker"),
                PathBuf::from("/opt/lemonfiber/stack/compose/plugins/komga.yml"),
                PathBuf::from("/opt/lemonfiber/stack/config/caddy/Caddyfile"),
                PathBuf::from("/opt/lemonfiber/stack/config/homepage/services.yaml"),
            ]
        );
    }

    /// The directories are mounted by the document, so they are made before it is
    /// written. A run that stopped between the two leaves directories nothing
    /// references, which is inert; the other order leaves an entry mounting a
    /// directory that is not there, which is a service that will not start.
    #[test]
    fn the_directories_a_document_mounts_are_written_before_the_document() {
        let planned = writes(&installed("komga", &["komga"]), stack());
        let document = planned.iter().position(|one| !one.is_directory());
        let first = document
            .and_then(|at| planned.get(at))
            .map(|one| matches!(one.lands, Lands::Document(_)));
        assert_eq!(
            first,
            Some(true),
            "the first thing after the directories is the document"
        );
        assert!(planned
            .iter()
            .take(document.unwrap_or_default())
            .all(super::Write::is_directory));
    }

    #[test]
    fn a_directory_carries_no_content_and_the_document_carries_the_container() {
        let planned = writes(&installed("komga", &["komga"]), stack());
        assert_eq!(
            planned.len(),
            4,
            "one household service is one directory, one document, and a region in each \
             of the proxy and the dashboard"
        );
        assert_eq!(planned.first().map(super::Write::is_directory), Some(true));
        assert_eq!(
            document(&planned),
            Some(crate::plugin::container::written(&installed(
                "komga",
                &["komga"]
            )))
        );
    }

    /// The content is the container derivation rather than a second rendering of it.
    /// Two renderings are free to disagree, and the one on disk is the one Compose
    /// reads — so a plugin could be shown one entry and run another.
    #[test]
    fn what_is_written_is_the_container_the_record_derives_and_not_a_copy_of_it() {
        let one = installed("komga", &["komga"]);
        let planned = writes(&one, stack());
        let written = document(&planned).unwrap_or_default();
        assert!(written.starts_with("services:\n"));
        assert!(written.contains("profiles: [plugin-komga]"));
    }

    /// Two plugins never write the same document. The name is the plugin's id, which
    /// the register already refuses to hold twice, so this is that rule showing up
    /// where it matters rather than a second one.
    #[test]
    fn two_plugins_write_two_documents() {
        assert_ne!(overlay(stack(), "komga"), overlay(stack(), "kavita"));
    }

    #[test]
    fn a_machine_with_nothing_installed_layers_no_documents() {
        assert!(documents(&[], stack()).is_empty());
    }

    /// The document is joined against whichever directory the invocation calls the
    /// project root, so an operator's own stack gets documents inside it rather than
    /// inside the one lemonfiber would have materialised.
    #[test]
    fn a_document_is_joined_against_the_root_it_is_given() {
        let theirs = Path::new("/srv/their-own-stack");
        assert_eq!(
            documents(&["komga".to_owned()], theirs),
            vec![PathBuf::from(
                "/srv/their-own-stack/compose/plugins/komga.yml"
            )]
        );
    }

    /// One document per installed plugin, in the register's own order, and each one
    /// exactly where that plugin's install wrote it.
    #[test]
    fn every_installed_plugin_contributes_the_document_its_install_wrote() {
        let held = ["komga".to_owned(), "kavita".to_owned()];
        assert_eq!(
            documents(&held, stack()),
            vec![overlay(stack(), "komga"), overlay(stack(), "kavita")]
        );
    }

    /// The register says what is layered, so a document nobody installed is not. A
    /// listing of the directory would run it; this cannot.
    #[test]
    fn a_document_no_plugin_is_registered_for_is_not_layered() {
        let layered = documents(&["komga".to_owned()], stack());
        assert!(!layered.contains(&overlay(stack(), "dropped-in-by-hand")));
        assert_eq!(layered.len(), 1);
    }
}
