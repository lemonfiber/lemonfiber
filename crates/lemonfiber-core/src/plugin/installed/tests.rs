use lemonfiber_plugin::Manifest;

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
fn each_ask_is_kept_on_the_service_that_makes_it() {
    let record = installed(|manifest| {
        manifest.asking = vec![
            lemonfiber_plugin::Ask {
                native: None,
                service: Some("komga-sidecar".to_owned()),
                capability: "library.curate".to_owned(),
                each: true,
                media_types: Some(vec!["tv".to_owned()]),
            },
            lemonfiber_plugin::Ask {
                native: None,
                service: Some("komga".to_owned()),
                capability: "download.usenet".to_owned(),
                each: false,
                media_types: None,
            },
        ];
    });
    let asks = |service| placed(record.as_ref(), service).map(|one| one.asks);
    let asking = |capability: &str, each, media_types: &[&str]| super::Asking {
        native: None,
        capability: capability.to_owned(),
        each,
        files: None,
        media_types: media_types.iter().map(|one| (*one).to_owned()).collect(),
    };
    assert_eq!(
        asks("komga-sidecar"),
        Some(vec![asking("library.curate", true, &["tv"])])
    );
    assert_eq!(
        asks("komga"),
        Some(vec![asking("download.usenet", false, &[])])
    );
}

#[test]
fn a_variable_asking_an_address_is_an_ask_once_for_each_medium_it_files() {
    use lemonfiber_manifest::container::{UrlOf, Value};
    let url = |capability: &str| {
        Value::Url(UrlOf {
            url_of: capability.to_owned(),
            files: None,
            native: None,
        })
    };
    let narrowed = |files: Option<&str>, native: Option<&str>| {
        Value::Url(UrlOf {
            url_of: "library.curate".to_owned(),
            files: files.map(str::to_owned),
            native: native.map(str::to_owned),
        })
    };
    let record = installed(|manifest| {
        manifest.asking = vec![lemonfiber_plugin::Ask {
            native: None,
            service: Some("komga".to_owned()),
            capability: "download.usenet".to_owned(),
            each: false,
            media_types: None,
        }];
        set(manifest, |service| {
            service.env = [
                ("USENET", url("download.usenet")),
                ("INDEXER", url("indexer.search")),
                ("AGAIN", url("indexer.search")),
                ("TV", narrowed(Some("tv"), None)),
                ("SERVARR", narrowed(None, Some("servarr"))),
                (
                    "PLAIN",
                    Value::Constant(lemonfiber_manifest::container::Constant::Flag(true)),
                ),
            ]
            .map(|(name, value)| (name.to_owned(), value))
            .into();
        });
    });
    let asked = |capability: &str| super::Asking {
        native: None,
        capability: capability.to_owned(),
        each: false,
        files: None,
        media_types: Vec::new(),
    };
    let curating = |files: Option<&str>, native: Option<&str>| super::Asking {
        native: native.map(lemonfiber_manifest::container::Apis::one),
        files: files.map(str::to_owned),
        ..asked("library.curate")
    };
    assert_eq!(
        placed(record.as_ref(), "komga").map(|one| one.asks),
        Some(vec![
            asked("download.usenet"),
            asked("indexer.search"),
            curating(None, Some("servarr")),
            curating(Some("tv"), None),
        ])
    );
}

#[test]
fn a_service_asking_nothing_records_no_asks_and_reads_back_from_a_record_without_them() {
    let record = whole();
    let written = record
        .as_ref()
        .and_then(|one| one.services.first())
        .and_then(|placed| serde_json::to_string(placed).ok())
        .unwrap_or_default();
    assert!(
        !written.is_empty() && !written.contains("asks"),
        "{written}"
    );
    let read: Option<Placed> = serde_json::from_str(&written).ok();
    assert_eq!(read.map(|one| one.asks), Some(Vec::new()));
}

/// The port a service answers on is what the record keeps, so whatever later asks for
/// what it fills can be told where it answers.
#[test]
fn the_port_a_service_answers_on_survives_the_install() {
    let record = installed(|manifest| {
        set(manifest, |service| {
            service.listens = Some(8080);
        });
    });

    assert_eq!(
        placed(record.as_ref(), "komga").and_then(|one| one.listens),
        Some(8080)
    );
    assert_eq!(
        placed(record.as_ref(), "komga-sidecar").and_then(|one| one.listens),
        None,
        "and the service that named none keeps none"
    );
}

#[test]
fn where_a_service_reaches_and_what_it_asks_for_there_survive_the_install() {
    let record = installed(|manifest| {
        set(manifest, |service| {
            service.reaches = Some("comicvine.gamespot.com".to_owned());
            service.asks_for = Some("Issue details".to_owned());
        });
    });

    assert_eq!(
        placed(record.as_ref(), "komga").map(|one| (one.reaches, one.asks_for)),
        Some((
            Some("comicvine.gamespot.com".to_owned()),
            Some("Issue details".to_owned())
        ))
    );
    assert_eq!(
        placed(record.as_ref(), "komga-sidecar").map(|one| (one.reaches, one.asks_for)),
        Some((None, None)),
        "and the service that said neither keeps neither"
    );
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
        demand: None,
        nonconforming: Vec::new(),
        proof: None,
        rehearsed: false,
        agreement: None,
        installed: whole().into_iter().collect(),
        install: None,
        removal: None,
        update: None,
        substituted: Vec::new(),
        sources: Vec::new(),
        unvetted: None,
        guards: Vec::new(),
        unproxied: false,
    };
    let done = Installs {
        demand: None,
        nonconforming: Vec::new(),
        proof: None,
        rehearsed: false,
        agreement: None,
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
                asks: Vec::new(),
                contests: Vec::new(),
                recipes_ran: Vec::new(),
                taking: Vec::new(),
                granting: Vec::new(),
                sharing: Vec::new(),
                waiting: Vec::new(),
                secured: None,
            })
        }),
        update: None,
        substituted: Vec::new(),
        sources: Vec::new(),
        unvetted: None,
        guards: Vec::new(),
        unproxied: false,
    };
    assert!(read.install.is_none());
    assert_eq!(done.install.map(|one| one.recorded), Some(true));
}

/// A record whose values this build would not have written is refused on the way in.
///
/// Each value is written into a Compose document, a proxy stanza or a dashboard entry
/// again every time the plugin's files are derived, long after the reader held the
/// manifest to the format. So each one is shown refused here, altered on a record
/// that otherwise reads back whole.
#[test]
fn a_record_carrying_a_value_this_build_would_not_have_written_is_refused() {
    assert!(whole().is_some(), "the fixture reads");
    for (what, alter) in alterations().into_iter().chain(landings()) {
        let Some(mut one) = whole() else {
            continue;
        };
        alter(&mut one);
        let mut register = Register::empty();
        assert_eq!(register.record(one), Ok(()), "{what}");
        let read = Register::parse(&register.to_json().unwrap_or_default());
        if what == "nothing altered" {
            assert_eq!(read, Ok(register), "the fixture itself reads back");
        } else {
            assert!(
                matches!(&read, Err(Unreadable::Damaged(why)) if why.contains("would not have written")),
                "{what}: {read:?}"
            );
        }
    }
}

/// Each value a record could carry that this build would not have written, with what it is.
fn alterations() -> [(&'static str, Alter); 14] {
    [
        ("an empty plugin id", |one| one.plugin = String::new()),
        ("a plugin id with a separator", |one| {
            one.plugin = "../x".to_owned();
        }),
        ("a service that is a parent link", |one| {
            each(one, |placed| placed.service = "..".to_owned());
        }),
        ("an image with a line break", |one| {
            each(one, |placed| {
                placed.image = "a/b\n    privileged: true".to_owned();
            });
        }),
        ("a digest that is not one", |one| {
            each(one, |placed| placed.digest = "latest".to_owned());
        }),
        ("a relative directory", |one| {
            each(one, |placed| placed.config_path = "config".to_owned());
        }),
        ("a directory leaving itself", |one| {
            each(one, |placed| placed.config_path = "/a/../..".to_owned());
        }),
        ("a directory outside its alphabet", |one| {
            each(one, |placed| placed.config_path = "/a:ro".to_owned());
        }),
        ("a directory at the root", |one| {
            each(one, |placed| placed.config_path = "/".to_owned());
        }),
        ("a hostname that is two labels", |one| {
            each(one, |placed| {
                if let Some(Reached::Household { hostname, .. }) = &mut placed.reached {
                    *hostname = "a.b".to_owned();
                }
            });
        }),
        ("a name the dashboard expands", |one| {
            each(one, |placed| placed.name = "{{KEY}}".to_owned());
        }),
        ("a description Compose expands", |one| {
            each(one, |placed| placed.description = "${KEY}".to_owned());
        }),
        ("a group the dashboard expands", |one| {
            each(one, |placed| {
                if let Some(Reached::Household { group, .. }) = &mut placed.reached {
                    *group = Some("{{KEY}}".to_owned());
                }
            });
        }),
        ("nothing altered", |_| ()),
    ]
}

/// Each path a record could land a mount or scratch space at that this build would not
/// have written, with what it is.
fn landings() -> [(&'static str, Alter); 6] {
    [
        ("a mount leaving its tree", |one| {
            each(one, |placed| {
                placed.fields.mount = vec![lemonfiber_manifest::container::Mount {
                    from: "../../etc".to_owned(),
                    to: "/etc-of-host".to_owned(),
                    ro: false,
                }];
            });
        }),
        ("a mount landing on the library", |one| {
            each(one, |placed| {
                placed.takes_data = true;
                placed.fields.mount = vec![lemonfiber_manifest::container::Mount {
                    from: "cache".to_owned(),
                    to: "/data/x".to_owned(),
                    ro: false,
                }];
            });
        }),
        ("two mounts from one source", |one| {
            each(one, |placed| {
                placed.fields.mount = ["/cache", "/again"]
                    .map(|to| lemonfiber_manifest::container::Mount {
                        from: "cache".to_owned(),
                        to: to.to_owned(),
                        ro: false,
                    })
                    .to_vec();
            });
        }),
        ("two mounts landing at one target", |one| {
            each(one, |placed| {
                placed.fields.mount = ["cache", "logs"]
                    .map(|from| lemonfiber_manifest::container::Mount {
                        from: from.to_owned(),
                        to: "/cache".to_owned(),
                        ro: false,
                    })
                    .to_vec();
            });
        }),
        ("scratch space over the configuration", |one| {
            each(one, |placed| {
                placed.fields.tmpfs = vec![placed.config_path.clone()];
            });
        }),
        ("scratch space with an option", |one| {
            each(one, |placed| {
                placed.fields.tmpfs = vec!["/tmp:exec".to_owned()];
            });
        }),
    ]
}

/// One change made to a record, for the case above.
type Alter = fn(&mut Installed);

/// Every service of a record, changed the same way.
fn each(one: &mut Installed, change: impl Fn(&mut Placed)) {
    one.services.iter_mut().for_each(change);
}

#[test]
fn the_shape_a_service_took_is_kept_and_a_record_without_one_reads_as_none() {
    let shaped = installed(|manifest| {
        set(manifest, |service| {
            service.shape = Some(lemonfiber_plugin::Shape::EgressGuard);
        });
    });
    let kept = shaped
        .as_ref()
        .and_then(|one| serde_json::to_string(one).ok())
        .unwrap_or_default();
    assert!(kept.contains(r#""shape":"egress-guard""#), "got: {kept}");
    let read = Register::parse(&format!(r#"{{"installed": [{kept}]}}"#));
    assert_eq!(
        read.ok()
            .and_then(|register| register.installed().first().cloned())
            .and_then(|one| one.services.first().and_then(|placed| placed.shape)),
        Some(lemonfiber_plugin::Shape::EgressGuard)
    );

    let plain = whole()
        .as_ref()
        .and_then(|one| serde_json::to_string(one).ok())
        .unwrap_or_default();
    assert!(!plain.contains(r#""shape""#), "got: {plain}");
    assert_eq!(
        placed(whole().as_ref(), "komga").and_then(|one| one.shape),
        None
    );
}

mod dashboard;

mod older;

mod reached;

mod speaking;
