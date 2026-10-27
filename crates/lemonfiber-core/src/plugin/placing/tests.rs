use std::path::{Path, PathBuf};

use super::{configuration, key_file, Lands, Reading, Write};
use crate::plugin::installed::{Installed, Placed, Reached};

/// The stack directory these are written beneath.
fn stack() -> &'static Path {
    Path::new("/opt/lemonfiber/stack")
}

/// The project an install writes, as the caller generated it.
const GENERATED: &str = "services:\n  komga: {}\n";

/// Where the project is kept beneath the stack directory.
fn project_at() -> PathBuf {
    stack().join(crate::project::FILE)
}

/// Everything installing `installed` writes, with the generated project among it.
fn writes(installed: &Installed, stack: &Path, reading: &Reading<'_>) -> Vec<Write> {
    super::writes(
        installed,
        stack,
        reading,
        Vec::new(),
        Some(Write::project(project_at(), None, GENERATED.to_owned())),
    )
}

/// One placed service, as a record holds one.
fn placed(service: &str) -> Placed {
    Placed {
        fields: lemonfiber_manifest::container::Fields::default(),
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
        icon: None,
        hidden: false,
        health: None,
        listens: None,
        media_types: Vec::new(),
        networks: Vec::new(),
        speaks: Vec::new(),
        fronts: None,
        upstream: std::collections::BTreeMap::new(),
        native: None,
        replaces: None,
        shape: None,
        asks: Vec::new(),
        reaches: None,
        asks_for: None,
        starting: None,
    }
}

/// An installed plugin holding the named services.
fn installed(plugin: &str, services: &[&str]) -> Installed {
    Installed {
        granted: Vec::new(),
        plugin: plugin.to_owned(),
        version: "1.2.0".to_owned(),
        name: None,
        description: None,
        services: services.iter().map(|one| placed(one)).collect(),
        provides: Vec::new(),
        contributions: Vec::new(),
        run_proofs: Vec::new(),
        request_proofs: Vec::new(),
        declared: crate::plugin::Declaration::default(),
        recipes: Vec::new(),
        from: String::new(),
        revision: String::new(),
        signed: String::new(),
        manifest: String::new(),
        tree: String::new(),
        installed_at: String::new(),
    }
}

#[test]
fn a_service_keeps_its_configuration_where_the_bundled_ones_keep_theirs() {
    assert_eq!(
        configuration(stack(), "komga"),
        PathBuf::from("/opt/lemonfiber/stack/config/komga")
    );
}

/// What the project in a plan is to hold.
fn document(planned: &[Write]) -> Option<String> {
    planned.iter().find_map(|one| match &one.lands {
        Lands::Document { text, .. } => Some(text.clone()),
        Lands::Directory { .. }
        | Lands::Key
        | Lands::Region { .. }
        | Lands::Secrets(_)
        | Lands::Starting { .. }
        | Lands::Upstream(_) => None,
    })
}

/// Every region a plan writes, as the file it goes in and whose it is.
fn regions(planned: &[Write]) -> Vec<(String, String)> {
    planned
        .iter()
        .filter_map(|one| match &one.lands {
            Lands::Region { key, owner, .. } => Some((key.clone(), owner.clone())),
            Lands::Directory { .. }
            | Lands::Document { .. }
            | Lands::Key
            | Lands::Secrets(_)
            | Lands::Starting { .. }
            | Lands::Upstream(_) => None,
        })
        .collect()
}

/// A household service gets a region in the proxy, the plugin's, after the document
/// that declares the service.
#[test]
fn a_household_service_is_written_into_the_proxy_after_its_document() {
    let planned = writes(&installed("komga", &["komga"]), stack(), &Reading::bare());

    assert_eq!(
        regions(&planned),
        vec![(crate::plugin::PROXY.to_owned(), "plugin komga".to_owned())]
    );
    assert!(planned
        .last()
        .is_some_and(|one| matches!(one.lands, Lands::Region { .. })));
}

/// The tier decides: an operator surface is not proxied, and neither is a service
/// nothing reaches, so a plugin of only those writes no region.
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

    assert!(regions(&writes(&plugin, stack(), &Reading::bare())).is_empty());
}

#[test]
fn a_plugin_whose_services_nothing_reaches_writes_into_neither() {
    let mut plugin = installed("komga", &["komga"]);
    let _ = plugin.services.first_mut().map(|one| one.reached = None);

    assert!(regions(&writes(&plugin, stack(), &Reading::bare())).is_empty());
}

#[test]
fn installing_writes_a_directory_for_each_service_and_the_project() {
    let planned = writes(
        &installed("komga", &["komga", "komga-worker"]),
        stack(),
        &Reading::bare(),
    );
    let paths: Vec<PathBuf> = planned.iter().map(|one| one.path.clone()).collect();
    assert_eq!(
        paths,
        vec![
            PathBuf::from("/opt/lemonfiber/stack/config/komga"),
            PathBuf::from("/opt/lemonfiber/stack/config/komga-worker"),
            PathBuf::from("/opt/lemonfiber/stack/compose.generated.yml"),
            PathBuf::from("/opt/lemonfiber/stack/config/caddy/Caddyfile"),
        ]
    );
}

/// The directories are mounted by the document, so they are made before it is
/// written. A run that stopped between the two leaves directories nothing
/// references, which is inert; the other order leaves an entry mounting a
/// directory that is not there, which is a service that will not start.
#[test]
fn the_directories_a_document_mounts_are_written_before_the_document() {
    let planned = writes(&installed("komga", &["komga"]), stack(), &Reading::bare());
    let document = planned.iter().position(|one| !one.is_directory());
    let first = document
        .and_then(|at| planned.get(at))
        .map(|one| matches!(one.lands, Lands::Document { .. }));
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
fn a_directory_carries_no_content_and_the_project_carries_what_was_generated() {
    let planned = writes(&installed("komga", &["komga"]), stack(), &Reading::bare());
    assert_eq!(
        planned.len(),
        3,
        "one household service is one directory, the project, and a region in the proxy"
    );
    assert_eq!(planned.first().map(super::Write::is_directory), Some(true));
    assert_eq!(document(&planned).as_deref(), Some(GENERATED));
}

/// An adapter service is given its key after the directories and before the document
/// that starts it, and a service that speaks nothing is given none.
#[test]
fn an_adapter_service_is_keyed_before_the_document_that_starts_it() {
    let mut plugin = installed("plex", &["plex", "plex-adapter"]);
    if let Some(adapter) = plugin.services.get_mut(1) {
        adapter.speaks = vec!["media.serve@1".to_owned()];
    }
    let planned = writes(&plugin, stack(), &Reading::bare());
    let at = |lands: fn(&Lands) -> bool| planned.iter().position(|one| lands(&one.lands));
    let key = planned
        .iter()
        .filter(|one| one.lands == Lands::Key)
        .map(|one| one.path.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        key,
        [PathBuf::from(
            "/opt/lemonfiber/stack/config/plex-adapter/lemonfiber.key"
        )]
    );
    assert_eq!(
        key_file(stack(), "plex-adapter"),
        key.first().cloned().unwrap_or_default()
    );
    assert!(at(|one| matches!(one, Lands::Directory { .. })) < at(|one| *one == Lands::Key));
    assert!(at(|one| *one == Lands::Key) < at(|one| matches!(one, Lands::Document { .. })));
}

#[test]
fn a_service_with_further_mounts_gets_its_mount_tree_and_each_writable_source_before_the_document()
{
    let mut plugin = installed("komga", &["komga"]);
    if let Some(service) = plugin.services.first_mut() {
        service.fields.mount = vec![
            lemonfiber_manifest::container::Mount {
                from: "cache".to_owned(),
                to: "/cache".to_owned(),
                ro: false,
            },
            lemonfiber_manifest::container::Mount {
                from: "Caddyfile".to_owned(),
                to: "/etc/caddy/Caddyfile".to_owned(),
                ro: true,
            },
        ];
    }
    let planned = writes(&plugin, stack(), &Reading::bare());
    let paths: Vec<PathBuf> = planned.iter().map(|one| one.path.clone()).collect();
    let tree = PathBuf::from("/opt/lemonfiber/stack/mounts/komga");
    let at = |path: &PathBuf| paths.iter().position(|one| one == path);
    let document = at(&project_at());
    assert!(at(&tree) < at(&tree.join("cache")));
    assert!(at(&tree.join("cache")) < document);
    assert_eq!(at(&tree.join("Caddyfile")), None);
    assert_eq!(
        super::mount_sources(&plugin, stack()),
        vec![tree.join("cache"), tree.join("Caddyfile")]
    );
}

#[test]
fn a_mount_the_model_refuses_plans_no_directory_and_is_no_source() {
    let mut plugin = installed("komga", &["komga"]);
    let mount = |from: &str, to: &str| lemonfiber_manifest::container::Mount {
        from: from.to_owned(),
        to: to.to_owned(),
        ro: false,
    };
    if let Some(service) = plugin.services.first_mut() {
        service.takes_data = true;
        service.fields.mount = vec![
            mount("../../etc", "/etc-of-host"),
            mount("/etc", "/absolute"),
            mount("library", "/data/x"),
            mount("a:b", "/colon"),
            mount("ok", "/c:ro"),
        ];
    }
    let tree = PathBuf::from("/opt/lemonfiber/stack/mounts/komga");
    let planned = writes(&plugin, stack(), &Reading::bare());
    assert!(
        planned.iter().all(|one| !one.path.starts_with(&tree)),
        "{planned:?}"
    );
    assert!(super::mount_sources(&plugin, stack()).is_empty());
}

#[test]
fn a_source_inside_another_is_planned_for_neither() {
    let mut plugin = installed("komga", &["komga"]);
    if let Some(service) = plugin.services.first_mut() {
        service.fields.mount = ["outer", "outer/inner"]
            .map(|from| lemonfiber_manifest::container::Mount {
                from: from.to_owned(),
                to: format!("/{}", from.replace('/', "-")),
                ro: false,
            })
            .to_vec();
    }
    let tree = PathBuf::from("/opt/lemonfiber/stack/mounts/komga");
    assert_eq!(
        super::mount_sources(&plugin, stack()),
        vec![tree.join("outer")]
    );
    let planned: Vec<PathBuf> = writes(&plugin, stack(), &Reading::bare())
        .into_iter()
        .map(|one| one.path)
        .filter(|path| path.starts_with(&tree))
        .collect();
    assert_eq!(planned, vec![tree.clone(), tree.join("outer")]);
}

#[test]
fn a_service_naming_a_secret_has_its_file_written_before_the_document_naming_it() {
    let mut plugin = installed("komga", &["komga", "reader"]);
    if let Some(first) = plugin.services.first_mut() {
        first.fields.env.insert(
            "KEY".to_owned(),
            lemonfiber_manifest::container::Value::Secret(
                lemonfiber_manifest::container::SecretOf {
                    secret: "key".to_owned(),
                },
            ),
        );
    }
    let file = |service: &str| Some(PathBuf::from(format!("/home/op/env/{service}.env")));
    let reading = Reading {
        secrets_file: &file,
        ..Reading::bare()
    };
    let planned = writes(&plugin, stack(), &reading);
    let at = |wanted: fn(&Lands) -> bool| planned.iter().position(|one| wanted(&one.lands));
    let secrets: Vec<&Write> = planned
        .iter()
        .filter(|one| matches!(one.lands, Lands::Secrets(_)))
        .collect();
    assert_eq!(
        secrets
            .iter()
            .map(|one| one.path.clone())
            .collect::<Vec<_>>(),
        [PathBuf::from("/home/op/env/komga.env")]
    );
    assert!(
        at(|lands| matches!(lands, Lands::Secrets(_)))
            < at(|lands| matches!(lands, Lands::Document { .. }))
    );
}
