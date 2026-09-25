use lemonfiber_plugin::{Bind, Manifest};

use super::super::register::{Register, Unreadable};
use super::super::reports::{Install, Installs};
use super::{Installed, Placed, Reached};

/// A plugin declaring two services, though the reader admits one at a time.
///
/// Two because what is under test is the record: with one service every
/// per-service decision is indistinguishable from a per-plugin one, and the two
/// here differ in exactly the ways the format exists to record — one faces the
/// household and the other is an operator surface, one keeps its state where the
/// convention says and the other says nothing at all.
const MANIFEST: &str = r#"
schema_version = 1

[plugin]
id          = "komga"
name        = "Komga"
version     = "1.2.0"
description = "Reads your comics on any browser"
without_it  = "Files on disk, no way to read them"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "komga"
name        = "Komga"
image       = "example.invalid/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.11.0"
port        = 25600
bind        = "lan"
criticality = "important"
takes_data  = true
config_path = "/config"

[[service]]
id          = "komga-sidecar"
name        = "Komga's indexer"
image       = "example.invalid/komga-sidecar"
digest      = "sha256:0000cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "0.4.1"
port        = 9000
bind        = "loopback"
criticality = "enhancing"

[[wiring]]
hostname        = "comics"
dashboard_group = "Library"
"#;

/// What installing the fixture settles, with whatever departure the case under
/// test needs made to the manifest first.
///
/// Nothing where the fixture stopped being a manifest, so a fixture that broke
/// fails the assertion it was written for rather than somewhere further down.
fn installed(change: impl FnOnce(&mut Manifest)) -> Option<Installed> {
    let mut manifest = Manifest::from_toml(MANIFEST).ok()?;
    change(&mut manifest);
    Some(Installed::of(&manifest))
}

/// **Every core capability its services fill, once each, and nothing namespaced.**
/// A capability of the plugin's own is inert by design — nothing asks for one, so
/// nothing can be left without it, and a removal naming one would be warning about
/// something nobody was reaching for.
#[test]
fn what_a_plugin_fills_is_the_core_names_its_services_provide() {
    let record = installed(|manifest| {
        for service in &mut manifest.services {
            service.provides = vec![
                "media.serve".to_owned(),
                "komga:kobo-sync".to_owned(),
                "media.serve".to_owned(),
            ];
        }
    });

    assert_eq!(
        record.as_ref().map(|one| one.provides.clone()),
        Some(vec!["media.serve".to_owned()]),
        "the core one, once, and not the plugin's own"
    );
    assert_eq!(
        whole().map(|one| one.provides),
        Some(Vec::new()),
        "and a plugin whose services fill nothing fills nothing"
    );
}

/// A change to the first service the fixture declares.
///
/// Reached rather than indexed: a fixture that stopped declaring a service would
/// otherwise end the run rather than fail the assertion it was written for.
fn set(manifest: &mut Manifest, change: impl FnOnce(&mut lemonfiber_plugin::Service)) {
    if let Some(service) = manifest.services.first_mut() {
        change(service);
    }
}

/// The fixture as its author wrote it.
fn whole() -> Option<Installed> {
    installed(|_| ())
}

/// How one of a record's services was placed.
fn placed(record: Option<&Installed>, service: &str) -> Option<Placed> {
    record?
        .services
        .iter()
        .find(|one| one.service == service)
        .cloned()
}

/// Where one of them keeps its own state.
fn config_path(record: Option<&Installed>, service: &str) -> Option<String> {
    placed(record, service).map(|one| one.config_path)
}

#[test]
fn the_configuration_directory_a_service_declares_survives_the_install() {
    assert_eq!(
        config_path(whole().as_ref(), "komga"),
        Some("/config".to_owned())
    );
}

/// The fallback is written down rather than left to be worked out again, which is
/// what makes the record answer where the directory *is*.
#[test]
fn a_service_that_declares_no_configuration_directory_records_the_published_one() {
    assert_eq!(
        config_path(whole().as_ref(), "komga-sidecar"),
        Some(lemonfiber_plugin::CONFIGURATION.to_owned())
    );
}

/// The manifest's own declaration, not a convention the record repeated. An
/// image keeping its state elsewhere is the whole reason the field exists.
#[test]
fn a_service_whose_image_reads_its_state_elsewhere_records_that_path() {
    let record = installed(|manifest| {
        set(manifest, |service| {
            service.config_path = Some("/app/data".to_owned());
        });
    });
    assert_eq!(
        config_path(record.as_ref(), "komga"),
        Some("/app/data".to_owned())
    );
}

#[test]
fn what_runs_is_recorded_as_the_path_and_the_digest_that_pins_it() {
    let one = placed(whole().as_ref(), "komga");
    assert_eq!(
        one.as_ref().map(|one| one.image.clone()),
        Some("example.invalid/komga".to_owned())
    );
    assert_eq!(
        one.as_ref().map(|one| one.digest.starts_with("sha256:")),
        Some(true)
    );
    assert_eq!(one.map(|one| one.tag), Some("1.11.0".to_owned()));
}

/// The tier is each service's own, read from the field that decides it rather
/// than from anything held beside the plugin.
#[test]
fn each_service_keeps_the_tier_its_own_declaration_names() {
    let record = whole();
    assert_eq!(
        placed(record.as_ref(), "komga").map(|one| one.reached),
        Some(Some(Reached::Household {
            port: 25600,
            hostname: "comics".to_owned(),
            group: Some("Library".to_owned()),
        }))
    );
    assert_eq!(
        placed(record.as_ref(), "komga-sidecar").map(|one| one.reached),
        Some(Some(Reached::Loopback {
            port: 9000,
            group: Some("Library".to_owned()),
        }))
    );
}

/// The correction that mattered: only the proxy is the wider tier's alone. An
/// operator surface still appears on the bundled dashboard, so the group and the
/// tier its link is rendered from both have to survive the install.
#[test]
fn a_loopback_service_keeps_the_dashboard_group_and_the_tier_its_link_is_built_from() {
    let one = placed(whole().as_ref(), "komga-sidecar").and_then(|one| one.reached);
    assert_eq!(one.as_ref().map(Reached::port), Some(9000));
    assert_eq!(one.as_ref().map(Reached::group), Some(Some("Library")));
    assert_eq!(one.as_ref().map(Reached::hostname), Some(None));
}

/// The three readings are the facts the arms carry, so a caller need not write
/// the match itself and get one arm of it wrong.
#[test]
fn what_a_household_service_is_reached_by_reads_the_same_either_way() {
    let one = placed(whole().as_ref(), "komga").and_then(|one| one.reached);
    assert_eq!(one.as_ref().map(Reached::port), Some(25600));
    assert_eq!(one.as_ref().map(Reached::hostname), Some(Some("comics")));
    assert_eq!(one.as_ref().map(Reached::group), Some(Some("Library")));
}

/// A label is a fact about one container. Defaulting to the plugin's id would
/// give two services of one plugin the same address.
#[test]
fn a_service_the_manifest_names_no_label_for_answers_on_its_own_id() {
    let record = installed(|manifest| manifest.wirings.clear());
    assert_eq!(
        placed(record.as_ref(), "komga").map(|one| one.reached),
        Some(Some(Reached::Household {
            port: 25600,
            hostname: "komga".to_owned(),
            group: None,
        }))
    );
}

/// The tier decides, and the record cannot say otherwise: a loopback service has
/// nowhere to put a hostname, whatever the manifest asked for.
#[test]
fn a_loopback_service_carries_no_hostname_even_where_one_is_declared() {
    let record = installed(|manifest| set(manifest, |service| service.bind = Some(Bind::Loopback)));
    assert_eq!(
        placed(record.as_ref(), "komga").map(|one| one.reached),
        Some(Some(Reached::Loopback {
            port: 25600,
            group: Some("Library".to_owned()),
        }))
    );
}

#[test]
fn a_service_with_no_listener_is_recorded_as_reached_by_nothing() {
    let record = installed(|manifest| {
        set(manifest, |service| {
            service.port = None;
            service.bind = None;
        });
    });
    assert_eq!(
        placed(record.as_ref(), "komga").map(|one| one.reached),
        Some(None)
    );
}

/// A port with no tier is a manifest the reader refuses. Were one to reach here
/// the record says it is reached by nothing rather than choosing a tier for it —
/// the one answer that cannot put an admin surface on the household network.
#[test]
fn a_port_with_no_tier_is_recorded_as_reached_by_nothing_rather_than_placed() {
    let record = installed(|manifest| set(manifest, |service| service.bind = None));
    assert_eq!(
        placed(record.as_ref(), "komga").map(|one| one.reached),
        Some(None)
    );
}

#[test]
fn whether_the_library_is_mounted_is_each_services_own_answer() {
    let record = whole();
    assert_eq!(
        placed(record.as_ref(), "komga").map(|one| one.takes_data),
        Some(true)
    );
    assert_eq!(
        placed(record.as_ref(), "komga-sidecar").map(|one| one.takes_data),
        Some(false)
    );
}

#[test]
fn the_plugin_is_recorded_under_its_id_and_the_version_that_was_installed() {
    let record = whole();
    assert_eq!(
        record
            .as_ref()
            .map(|one| (one.plugin.clone(), one.version.clone(), one.services.len())),
        Some(("komga".to_owned(), "1.2.0".to_owned(), 2))
    );
}

/// A register holding whatever the fixture settles, for the cases below.
fn register(plugins: &[&str]) -> Register {
    let mut register = Register::empty();
    for id in plugins {
        let one = installed(|manifest| manifest.plugin.id = (*id).to_owned());
        assert_eq!(one.map(|one| register.record(one)), Some(Ok(())));
    }
    register
}

#[test]
fn a_record_written_is_a_record_read_back() {
    let register = register(&["komga"]);
    let text = register.to_json().unwrap_or_default();
    assert_eq!(Register::parse(&text), Ok(register));
}

#[test]
fn a_machine_with_no_record_has_nothing_installed() {
    assert_eq!(Register::parse(""), Ok(Register::empty()));
    assert_eq!(Register::parse("   \n"), Ok(Register::empty()));
    assert!(Register::empty().installed().is_empty());
    assert_eq!(Register::empty().holds("komga"), None);
}

/// The gate this record exists to pass. Every other small record beside the
/// settings reads a damaged file as its default; this one must not, because the
/// default is *nothing is installed* and that is a false answer about somebody
/// else's service running on the machine.
#[test]
fn a_damaged_record_is_refused_rather_than_read_as_nothing_installed() {
    let refused = Register::parse("{ not json at all");
    assert!(
        matches!(refused, Err(Unreadable::Damaged(_))),
        "got: {refused:?}"
    );
    let said = refused.err().map(|why| why.to_string()).unwrap_or_default();
    assert!(said.contains("could not be read"), "got: {said}");
}

/// A field this build does not know is a record something else wrote, and
/// reading the half it recognises would report a narrower install than happened.
#[test]
fn a_record_carrying_something_this_build_does_not_know_is_refused() {
    let refused = Register::parse(r#"{"installed": [], "running": true}"#);
    assert!(
        matches!(refused, Err(Unreadable::Damaged(_))),
        "got: {refused:?}"
    );
}

#[test]
fn a_record_naming_one_plugin_twice_is_refused_naming_it() {
    let one = whole()
        .as_ref()
        .and_then(|one| serde_json::to_string(one).ok())
        .unwrap_or_default();
    let refused = Register::parse(&format!(r#"{{"installed": [{one}, {one}]}}"#));
    assert_eq!(refused, Err(Unreadable::Twice("komga".to_owned())));
    let said = refused.err().map(|why| why.to_string()).unwrap_or_default();
    assert!(said.contains("komga"), "got: {said}");
    assert!(said.contains("twice"), "got: {said}");
}

/// An install over an install is an update, which reverses one set of changes and
/// applies another. Writing it as an install would leave the record describing
/// one version and the machine carrying two.
#[test]
fn installing_over_an_install_is_refused_naming_what_is_there() {
    let mut register = register(&["komga"]);
    let said = whole()
        .and_then(|one| register.record(one).err())
        .map(|why| why.to_string())
        .unwrap_or_default();
    assert!(said.contains("komga"), "got: {said}");
    assert!(said.contains("1.2.0"), "got: {said}");
    assert_eq!(register.installed().len(), 1);
}

/// The file reads the same twice, so a diff of it says what changed rather than
/// where somebody appended.
#[test]
fn the_record_keeps_its_plugins_in_one_order_whatever_order_they_arrived_in() {
    let one = register(&["plex", "komga", "uptime"]);
    assert_eq!(one, register(&["uptime", "plex", "komga"]));
    let held: Vec<&str> = one
        .installed()
        .iter()
        .map(|record| record.plugin.as_str())
        .collect();
    assert_eq!(held, vec!["komga", "plex", "uptime"]);
}

#[test]
fn what_is_recorded_for_one_plugin_is_answerable_by_name() {
    let register = register(&["komga"]);
    assert_eq!(
        register.holds("komga").map(|one| one.version.as_str()),
        Some("1.2.0")
    );
    assert_eq!(register.holds("plex"), None);
}

/// The report carries both halves whichever way it was reached, so a surface has
/// one shape to render rather than two that agree today.
#[test]
fn the_report_says_what_is_installed_and_what_this_run_did() {
    let read = Installs {
        installed: whole().into_iter().collect(),
        install: None,
        removal: None,
        update: None,
        substituted: Vec::new(),
    };
    let done = Installs {
        removal: None,
        installed: whole().into_iter().collect(),
        install: whole().map(|would| {
            Box::new(Install {
                would,
                recorded: true,
                changes: Vec::new(),
                proofs: Vec::new(),
                against: None,
                verified: None,
                overrides: Vec::new(),
                reversed: None,
                contests: Vec::new(),
            })
        }),
        update: None,
        substituted: Vec::new(),
    };
    assert!(read.install.is_none());
    assert_eq!(done.install.map(|one| one.recorded), Some(true));
}
