//! The container lemonfiber writes for a plugin's service, and the whole of what
//! that service may reach of the machine.
//!
//! A plugin supplies no container definition, and lemonfiber writes one from what it
//! declared. The two halves are one rule rather than two: a format that refused a
//! supplied fragment and then let a declaration widen the generated one would be
//! refusing the paperwork rather than the reach. So the set of fields a manifest may
//! carry *is* the set of things a plugin may ask for, and this is the only place that
//! set is turned into a container.
//!
//! **Everything here is derived and nothing here is remembered.** The profile is the
//! plugin's id with a word in front of it; the interface behind a published port is
//! the tier rendered by lemonfiber's own rule; the source of the configuration mount
//! is lemonfiber's own directory named for the service. None of those is in the
//! install record, because a copy of a derivation is free to disagree with the
//! derivation — so the record holds what was decided and this holds what follows
//! from it.
//!
//! **The mount set is fixed, and that is the whole of what makes the question
//! answerable.** A service gets its own configuration directory, and the library
//! where it declared it needs one. There is no third, and no field anywhere in the
//! format by which a third could be asked for — which is why "what can this plugin
//! reach" is answered by reading the format rather than by inspecting the machine it
//! was installed on.
//!
//! **What refuses a manifest is the reader, once.** This writes the entry the record
//! describes rather than forming a second opinion about it: the values were held to
//! the format when the install was settled, and a rule applied again here would refuse
//! a plugin at a point where nobody is looking at a manifest and nothing could say
//! which line of it was wrong. A record this build would not have written is the
//! register's problem, and it refuses one it cannot read rather than reading it as
//! nothing.
//!
//! **It extends `defaults` rather than `rootless`, and that is a decision rather
//! than an omission.** The bundled stack picks between the two per image, because
//! the pair a `rootless` service is told is read by some images and silently ignored
//! by others — and setting them on an image that ignores them is a no-op that reads
//! like a security control. A plugin has no field by which to say which shape its
//! image is, so writing `rootless` for it would be this build asserting something
//! about a stranger's image that nobody checked.

use std::fmt::Write as _;

use super::installed::{Installed, Placed, Reached};

/// The shared template every bundled service extends, and the service inside it.
///
/// Named as a pair because the two are one reference: `extends` without the file is
/// a service in the same document, and the template deliberately lives in a file no
/// include list names so that it can never become a container of its own.
const TEMPLATE: (&str, &str) = ("compose/_common.yml", "defaults");

/// The word in front of a plugin's id, which is the profile its services sit in.
///
/// Its own profile rather than one the stack already has: a profile is the unit that
/// starts and stops together, and a stranger's service joining `tv` would mean
/// `lemonfiber up tv` could no longer be described without naming what is installed.
const PROFILE: &str = "plugin-";

/// The Compose profile this plugin's services sit in.
///
/// Published because an install has to start exactly them and a removal has to take
/// exactly them back, and a caller that spelled the name itself would be a second
/// answer to which profile the entry above was written into.
#[must_use]
pub fn profile(plugin: &str) -> String {
    format!("{PROFILE}{plugin}")
}

/// Where lemonfiber keeps a service's own configuration directory.
///
/// The source is lemonfiber's and the target is the plugin's: an image that reads
/// its configuration somewhere other than the convention says where, and gets the
/// same one directory mounted where it actually reads it.
const CONFIGURATION: &str = "./config";

/// The library, mounted exactly as every bundled service mounts it.
const LIBRARY: &str = "${DATA_ROOT:-./data}:/data";

/// The interface a household port is published on.
///
/// The same variable and the same fallback the bundled stack's own household
/// services are written with. A second spelling here would be a plugin's service
/// published on terms a bundled one is not, which is the thing the tier exists to
/// prevent.
const HOUSEHOLD: &str = "${LAN_BIND:-0.0.0.0}";

/// The interface an operator surface is published on.
const OPERATOR: &str = "127.0.0.1";

/// The container entries lemonfiber writes for an installed plugin.
///
/// One document rather than one entry, because a plugin is several services in the
/// contract even where this generation of the format admits one — and a caller
/// handed a single entry would be a caller that has to learn to join them the day
/// the second arrives.
///
/// A function of the record alone. Everything it needs was settled when the plugin
/// was installed, so this answers the same on a machine whose plugin source is long
/// gone as on the one that installed it.
#[must_use]
pub fn written(installed: &Installed) -> String {
    let mut document = String::from("services:\n");
    for placed in &installed.services {
        document.push_str(&entry(&installed.plugin, placed));
    }
    document
}

/// One service's entry, in the shape the bundled fragments are written in.
///
/// The digest rather than the tag, joined to the image by the `@` a registry reads:
/// the tag is a name its publisher can repoint, so an entry written from one could
/// run something other than what was reviewed with nothing in the manifest having
/// changed.
fn entry(plugin: &str, placed: &Placed) -> String {
    let (file, service) = TEMPLATE;
    let mut written = String::new();
    let _ = write!(
        written,
        "  {}:\n    extends:\n      file: {file}\n      service: {service}\n",
        placed.service
    );
    let _ = writeln!(written, "    image: {}@{}", placed.image, placed.digest);
    let _ = writeln!(written, "    profiles: [{}]", profile(plugin));
    if let Some(reached) = &placed.reached {
        let port = reached.port();
        let _ = writeln!(
            written,
            "    ports: [\"{}:{port}:{port}\"]",
            published(reached)
        );
    }
    written.push_str("    volumes:\n");
    if placed.takes_data {
        let _ = writeln!(written, "      - {LIBRARY}");
    }
    let _ = writeln!(
        written,
        "      - {CONFIGURATION}/{}:{}",
        placed.service, placed.config_path
    );
    written
}

/// Which interface a tier publishes on.
///
/// Read off the tier and never off a field, because there is no field: a plugin that
/// could write its own interface could put a full-control surface on the household
/// network without touching anything the web-security checks inspect.
const fn published(reached: &Reached) -> &'static str {
    match reached {
        Reached::Loopback { .. } => OPERATOR,
        Reached::Household { .. } => HOUSEHOLD,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use lemonfiber_plugin::Manifest;

    use super::super::installed::{Installed, Placed, Reached};
    use super::{entry, published, written, CONFIGURATION, HOUSEHOLD, LIBRARY, OPERATOR, PROFILE};

    /// The template the generated entries extend, as the stack ships it.
    ///
    /// The file itself rather than a copy of it, so the mount reading below follows
    /// the same `extends` a container engine would follow on the operator's machine.
    const COMMON: &str = include_str!("../../../../assets/media-stack/compose/_common.yml");

    /// The digest the fixtures pin, written once so the entry and the manifest below
    /// cannot pin different things while claiming to describe one install.
    const DIGEST: &str = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945";

    /// The entry a household service that handles media is written as.
    const HOUSEHOLD_ENTRY: &str = "services:\n  \
         komga:\n    \
         extends:\n      \
         file: compose/_common.yml\n      \
         service: defaults\n    \
         image: ghcr.io/gotson/komga@sha256:\
         4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945\n    \
         profiles: [plugin-komga]\n    \
         ports: [\"${LAN_BIND:-0.0.0.0}:25600:25600\"]\n    \
         volumes:\n      \
         - ${DATA_ROOT:-./data}:/data\n      \
         - ./config/komga:/config\n";

    /// One service, as installing a plugin settles it.
    fn placed() -> Placed {
        Placed {
            service: "komga".to_owned(),
            image: "ghcr.io/gotson/komga".to_owned(),
            digest: DIGEST.to_owned(),
            tag: "1.11.0".to_owned(),
            config_path: "/config".to_owned(),
            takes_data: true,
            reached: Some(Reached::Household {
                port: 25600,
                hostname: "comics".to_owned(),
                group: Some("Library".to_owned()),
            }),
        }
    }

    /// The record one such service is held in.
    fn installed(services: Vec<Placed>) -> Installed {
        Installed {
            plugin: "komga".to_owned(),
            version: "1.0.0".to_owned(),
            services,
        }
    }

    /// The document written for one placed service.
    fn document(one: Placed) -> String {
        written(&installed(vec![one]))
    }

    /// The whole entry, written out once so a reader can see what is claimed below.
    ///
    /// Asserted whole rather than line by line. What this generates is a file a
    /// container engine reads, and a test that checked each line for a substring
    /// would pass on a document whose lines are in an order Compose does not accept.
    #[test]
    fn the_entry_is_the_one_the_contract_describes() {
        assert_eq!(document(placed()), HOUSEHOLD_ENTRY);
    }

    /// A manifest this build installs, for the one test that joins the two halves.
    const INSTALLABLE: &str = r#"
schema_version = 1

[plugin]
id          = "komga"
name        = "Komga"
version     = "1.0.0"
description = "Reads your comics on any screen"
without_it  = "Comics on disk, nothing to read them with"
upstream    = "https://github.com/gotson/komga"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "komga"
name        = "Komga"
image       = "ghcr.io/gotson/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.11.0"
port        = 25600
bind        = "lan"
criticality = "enhancing"
takes_data  = true
config_path = "/config"

[[wiring]]
hostname        = "comics"
dashboard_group = "Library"
"#;

    /// The entry is what installing an actual manifest comes to.
    ///
    /// Everything else here is asked of records built by hand, which is what lets the
    /// generator be put in front of shapes a manifest cannot currently produce. This
    /// one is the join: without it the generator could be written against a record no
    /// install ever settles, and every test above would still pass.
    #[test]
    fn the_entry_is_written_from_what_installing_a_manifest_settles() {
        let written_for = Manifest::from_toml(INSTALLABLE)
            .ok()
            .map(|manifest| written(&Installed::of(&manifest)));
        assert_eq!(written_for.as_deref(), Some(HOUSEHOLD_ENTRY));
    }

    /// The digest is what runs, and the tag never reaches the entry.
    ///
    /// Both halves, because a generator that wrote neither would pass a test that
    /// only looked for the tag's absence.
    #[test]
    fn what_runs_is_the_digest_and_never_the_tag() {
        let entry = document(placed());
        assert!(entry.contains(&format!("@{DIGEST}")), "got: {entry}");
        assert!(!entry.contains("1.11.0"), "got: {entry}");
    }

    /// An operator surface is published where the stack publishes its own.
    ///
    /// The address is this build's answer to the tier rather than anything a manifest
    /// said: the only thing changed here is the tier.
    #[test]
    fn each_tier_is_published_on_the_interface_the_tier_decides() {
        assert!(document(placed()).contains(&format!("{HOUSEHOLD}:25600:25600")));

        let operator = document(Placed {
            reached: Some(Reached::Loopback {
                port: 25600,
                group: None,
            }),
            ..placed()
        });
        assert!(
            operator.contains(&format!("{OPERATOR}:25600:25600")),
            "got: {operator}"
        );
        assert!(!operator.contains(HOUSEHOLD), "got: {operator}");
    }

    /// A service with no listener is written with no ports at all.
    ///
    /// Not a port mapping onto nothing, and not an empty list: a service that
    /// publishes nothing is a different entry rather than the same one with a blank
    /// in it.
    #[test]
    fn a_service_with_no_listener_publishes_nothing() {
        let entry = document(Placed {
            reached: None,
            ..placed()
        });
        assert!(!entry.contains("ports:"), "got: {entry}");
        assert!(entry.contains("volumes:"), "got: {entry}");
    }

    /// The library is mounted for a service that said it handles media, and no other.
    #[test]
    fn the_library_is_mounted_only_where_the_manifest_said_it_is_wanted() {
        assert!(document(placed()).contains(LIBRARY));

        let without = document(Placed {
            takes_data: false,
            ..placed()
        });
        assert!(!without.contains(LIBRARY), "got: {without}");
        assert!(
            without.contains("./config/komga:/config"),
            "its own directory is mounted either way: {without}"
        );
    }

    /// An image that keeps its configuration elsewhere gets the same one directory,
    /// mounted where it actually reads it.
    ///
    /// The source is lemonfiber's on both readings and there is one of it on both:
    /// the target is the only thing the manifest moved.
    #[test]
    fn a_declared_configuration_directory_moves_the_target_and_nothing_else() {
        let entry = document(Placed {
            config_path: "/app/data".to_owned(),
            ..placed()
        });
        assert!(
            entry.contains(&format!("{CONFIGURATION}/komga:/app/data")),
            "got: {entry}"
        );
        assert_eq!(mounts(&entry).len(), 2, "still two mounts: {entry}");
        assert_eq!(mounts(&document(placed())).len(), 2);
    }

    /// The profile is the plugin's own, so what `up` starts stays describable.
    #[test]
    fn the_service_sits_in_a_profile_named_for_the_plugin() {
        assert!(document(placed()).contains(&format!("profiles: [{PROFILE}komga]")));
    }

    /// A plugin that places nothing writes a document with nothing in it, rather
    /// than a document that is not one.
    #[test]
    fn a_plugin_that_places_nothing_writes_a_document_with_nothing_in_it() {
        assert_eq!(written(&installed(Vec::new())), "services:\n");
    }

    /// Several services are several entries under one heading.
    ///
    /// One is what this generation of the format admits and the record is shaped for
    /// what the contract describes, so the joining is written and checked before
    /// there is a manifest that needs it — a caller handed one entry would be a
    /// caller that has to learn to join them the day the second arrives.
    #[test]
    fn a_plugin_placing_two_services_writes_both_under_one_heading() {
        let second = Placed {
            service: "komga-sync".to_owned(),
            takes_data: false,
            reached: None,
            ..placed()
        };
        let both = written(&installed(vec![placed(), second]));
        assert_eq!(both.matches("services:").count(), 1, "got: {both}");
        assert!(both.contains("  komga:\n"), "got: {both}");
        assert!(both.contains("  komga-sync:\n"), "got: {both}");
        assert!(
            both.contains("      - ./config/komga-sync:/config"),
            "each gets its own directory: {both}"
        );
        assert_eq!(
            both.matches("profiles: [plugin-komga]").count(),
            2,
            "and both sit in the plugin's one profile: {both}"
        );
    }

    /// One entry is the document without the heading, so the two cannot drift.
    #[test]
    fn the_document_is_its_entries_under_one_heading() {
        let record = installed(vec![placed()]);
        let each: String = record
            .services
            .iter()
            .map(|one| entry(&record.plugin, one))
            .collect();
        assert_eq!(written(&record), format!("services:\n{each}"));
    }

    /// The keys an entry may carry, and the whole of what it can ever carry.
    ///
    /// What a plugin may not have, read from the other end: a mount of its own, a
    /// device, a kernel capability, a network of its own, a privileged container, a
    /// user override, an entrypoint, a command and an environment variable are each
    /// a key that is not in this set, so a generator that grew one would fail here
    /// rather than on somebody's machine.
    const KEYS: &[&str] = &["extends", "image", "profiles", "ports", "volumes"];

    /// Every key one generated entry carries, read off the document itself.
    ///
    /// Read rather than parsed. What is asserted is what is actually written, and a
    /// YAML reader would answer about the document it made of the text rather than
    /// about the text — which is the half a container engine will disagree with.
    fn keys(document: &str) -> BTreeSet<String> {
        document
            .lines()
            .filter(|line| line.starts_with("    ") && !line.starts_with("     "))
            .map(|line| {
                let line = line.trim();
                line.split(':').next().unwrap_or(line).to_owned()
            })
            .collect()
    }

    /// Every mount one generated entry carries.
    fn mounts(document: &str) -> Vec<String> {
        document
            .lines()
            .filter_map(|line| line.trim().strip_prefix("- ").map(str::to_owned))
            .collect()
    }

    /// Every shape an entry can be written in, so a key written only for one of them
    /// is not the one a single reading misses.
    fn shapes() -> Vec<String> {
        vec![
            document(placed()),
            document(Placed {
                reached: Some(Reached::Loopback {
                    port: 25600,
                    group: None,
                }),
                ..placed()
            }),
            document(Placed {
                reached: None,
                ..placed()
            }),
            document(Placed {
                takes_data: false,
                ..placed()
            }),
            document(Placed {
                config_path: "/app/data".to_owned(),
                ..placed()
            }),
        ]
    }

    /// Nothing outside the permitted set is ever written.
    #[test]
    fn a_generated_entry_carries_no_key_outside_the_permitted_set() {
        let permitted: BTreeSet<String> = KEYS.iter().map(|&key| key.to_owned()).collect();
        for entry in shapes() {
            let carried = keys(&entry);
            assert!(!carried.is_empty(), "something was read: {entry}");
            assert!(
                carried.is_subset(&permitted),
                "carries a key outside the set: {carried:?} in {entry}"
            );
        }
    }

    /// The reading above is shown finding a key that should not be there.
    ///
    /// Without this the gate is a measurement nobody has watched succeed: a reader
    /// that always answered with the permitted set would pass every assertion above
    /// and would notice nothing it exists to notice. The keys planted are the ones
    /// a plugin may not have, each on an entry otherwise exactly as generated.
    #[test]
    fn the_reading_finds_a_key_the_permitted_set_does_not_carry() {
        let permitted: BTreeSet<String> = KEYS.iter().map(|&key| key.to_owned()).collect();
        for reach in [
            "    cap_add: [NET_ADMIN]",
            "    devices: [/dev/net/tun]",
            "    network_mode: host",
            "    privileged: true",
            "    user: \"0:0\"",
            "    entrypoint: /bin/sh",
            "    command: -c 'cat /etc/shadow'",
            "    environment: {TZ: UTC}",
        ] {
            let planted = format!("{}{reach}\n", document(placed()));
            assert!(
                !keys(&planted).is_subset(&permitted),
                "a planted {reach} is found: {planted}"
            );
        }
    }

    /// What the stack's own mount reader makes of a generated document.
    ///
    /// A second reader written here would answer about a slightly different set of
    /// declarations than the one the shipped stack is checked with, and the two
    /// would drift without either being wrong. The template is given alongside,
    /// because a service's mounts are what it declares plus whatever it extends.
    fn crowded(document: &str) -> Vec<String> {
        crate::stack::mounts::crowded(&[
            (PathBuf::from("compose/_common.yml"), COMMON.to_owned()),
            (PathBuf::from("compose/plugins.yml"), document.to_owned()),
        ])
        .into_iter()
        .map(|one| one.service)
        .collect()
    }

    /// Every shape of generated entry sees one mount beneath the library.
    #[test]
    fn a_generated_entry_sees_one_mount_beneath_the_library() {
        for entry in shapes() {
            let found = crowded(&entry);
            assert!(
                found.is_empty(),
                "{found:?} sees more than one mount beneath the library in {entry}"
            );
        }
    }

    /// And that reader is shown refusing a second mount beneath the library.
    ///
    /// Planted on the generated entry rather than on a document written for the
    /// occasion, so what is refused is this generator's own output with one line
    /// added — which is the shape the rule exists to catch.
    #[test]
    fn a_second_mount_beneath_the_library_is_found_by_that_reader() {
        let planted = document(placed()).replace(
            "      - ./config/komga:/config\n",
            "      - ${DATA_ROOT:-./data}/comics:/comics\n      - ./config/komga:/config\n",
        );
        assert_eq!(crowded(&planted), vec!["komga".to_owned()]);
    }

    /// Both tiers render to an interface, and to different ones.
    ///
    /// Asked of the function directly as well as through a document, because the two
    /// arms are the whole of the rule and an arm nothing enters is an answer nobody
    /// has read.
    #[test]
    fn each_tier_renders_to_its_own_interface() {
        assert_eq!(
            published(&Reached::Loopback {
                port: 1,
                group: None
            }),
            OPERATOR
        );
        assert_eq!(
            published(&Reached::Household {
                port: 1,
                hostname: "comics".to_owned(),
                group: None
            }),
            HOUSEHOLD
        );
        assert_ne!(OPERATOR, HOUSEHOLD);
    }
}
