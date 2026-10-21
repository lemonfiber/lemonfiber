//! Which interface the stack this binary carries publishes each port on.
//!
//! Read from the project generated from the embedded copy — the same `STACK` the binary
//! runs — rather than from the checked-out submodule. Those are the entries a machine
//! ends up running, and the directory beside them belongs to another repository that a
//! clone need not have populated at all. The bytes are fixed into this test binary when
//! it is compiled, and `build.rs` refuses to produce a build without them, so there is
//! no arrangement in which this reads nothing and finds nothing to object to.

use std::collections::BTreeSet;
use std::net::IpAddr;

use lemonfiber::carried::STACK;
use lemonfiber_core::config::env::EnvFile;
use lemonfiber_manifest::{Bind, Service};

/// The address a service reachable only from the host machine is published on.
const LOOPBACK: &str = "127.0.0.1";

/// An address the operator narrowed the household tier to, which no mapping writes
/// unless the one setting brought it there.
///
/// The setting is a knob rather than an address, because the stack has no way to learn
/// which of a host's addresses is the LAN one: that varies by machine, changes with
/// DHCP, and is different again on a laptop that moves. So the default is every
/// interface and the operator is told so, which is the honest half of the trade — and
/// every household mapping has to be written from it, so narrowing it narrows all of
/// them at once.
const NARROWED: &str = "192.0.2.7";

/// The file the stack ships its settings in, which an operator's own starts from.
const SETTINGS_FILE: &str = ".env.example";

/// The setting itself, as that file names it.
///
/// Without the shape a mapping wraps it in, because what is read there is a value
/// rather than an expansion.
const SETTING: &str = "LAN_BIND";

/// The services published beyond loopback, and why each one is.
///
/// Everything else in the stack is an admin surface, opened from the machine
/// running it or through a tunnel the operator already has. These are the ones a
/// household reaches from a device that is not that machine, and reaching them is
/// the whole of the case for putting them on the network.
///
/// A service that arrives publishing on anything but loopback and is not written
/// down here fails, and the second half of the entry is where somebody has to say
/// what it is for. The list is the decision; this is where it is reviewable.
const HOUSEHOLD: &[(&str, &str)] = &[
    (
        "door",
        "a television plays the library, and a television is not the host — through the \
         guard that asks the media server about every byte, which is published in its place",
    ),
    (
        "seerr",
        "the household asks for things from their own phones",
    ),
    (
        "calibre-web-automated",
        "an e-reader fetches books over the network it is already on",
    ),
    (
        "audiobookshelf",
        "a phone plays audiobooks somewhere away from the desk",
    ),
    (
        "homepage",
        "the page linking the household services is opened wherever they are",
    ),
    (
        "decline",
        "somebody invited turns the invitation down from their own device",
    ),
    (
        "caddy",
        "the hostnames and certificates every other household service is reached by",
    ),
    (
        "navidrome",
        "a speaker in another room plays the music, and the speaker is not the host",
    ),
];

/// One published port: which service publishes it, on what address, and where
/// that is written.
struct Publication {
    /// The Compose service carrying the `ports:` entry.
    service: String,
    /// The port on the host, which is the one an operator types.
    port: u16,
    /// The address it is published on, empty where none was written down.
    address: String,
    /// The file it was found in, so a failure names something to open.
    file: String,
}

/// Every port the project generated from the embedded stack publishes, where the
/// operator's environment file holds `operator`.
fn publications(operator: &dyn Fn(&str) -> Option<String>) -> Vec<Publication> {
    let document = generated_reading(operator);
    let mut found = Vec::new();
    let services = document
        .get("services")
        .and_then(serde_norway::Value::as_mapping)
        .into_iter()
        .flatten();
    for (service, entry) in services {
        let ports = entry
            .get("ports")
            .and_then(serde_norway::Value::as_sequence)
            .into_iter()
            .flatten()
            .filter_map(serde_norway::Value::as_str);
        found.extend(
            ports
                .filter_map(mapping)
                .map(|(port, address)| Publication {
                    service: service.as_str().unwrap_or_default().to_owned(),
                    port,
                    address,
                    file: lemonfiber_core::project::FILE.to_owned(),
                }),
        );
    }
    assert!(
        !found.is_empty(),
        "the embedded stack publishes no ports at all, which means this is reading the wrong thing"
    );
    found
}

/// The host port and address one mapping publishes on.
///
/// A mapping is `[address:]host:container`. With no address Compose publishes on
/// every interface, and that is reported as an empty address rather than passed
/// over — a mapping written without one is the exact defect this is here for.
fn mapping(spec: &str) -> Option<(u16, String)> {
    let parts: Vec<&str> = spec.split(':').collect();
    let mut tail = parts.iter().rev();
    let container = tail.next()?;
    let host = tail.next()?;
    if !container.split('/').next().is_some_and(numeric) {
        return None;
    }
    let port: u16 = host.parse().ok()?;
    let address = parts
        .iter()
        .take(parts.len().saturating_sub(2))
        .copied()
        .collect::<Vec<&str>>()
        .join(":");
    Some((port, address))
}

/// Whether a word is a number, having any characters at all.
fn numeric(word: &str) -> bool {
    !word.is_empty() && word.chars().all(|digit| digit.is_ascii_digit())
}

/// The manifest the embedded stack declares itself with, read the way the binary reads
/// it.
fn manifest() -> lemonfiber_manifest::Manifest {
    let Ok(manifest) = lemonfiber_core::stack::Source::Embedded(&STACK).manifest() else {
        unreachable!("the build refuses to produce this binary against a manifest it cannot read")
    };
    manifest
}

/// The project generated from the embedded stack, as Compose would read it.
pub(super) fn generated() -> serde_norway::Value {
    generated_reading(&|_| None)
}

/// The project generated from the embedded stack, where the operator's environment file
/// holds `operator`.
fn generated_reading(operator: &dyn Fn(&str) -> Option<String>) -> serde_norway::Value {
    let project = lemonfiber_core::project::generated(
        &manifest(),
        &[],
        &lemonfiber_core::wiring::Chosen::read(None),
        &lemonfiber_core::project::Reading {
            operator,
            ..lemonfiber_core::project::Reading::bare()
        },
    );
    serde_norway::from_str(&project.text).unwrap_or_default()
}

/// What an operator's environment file holds where they narrowed the household tier.
fn narrowed(key: &str) -> Option<String> {
    (key == SETTING).then(|| NARROWED.to_owned())
}

/// Every service the embedded stack declares.
fn declared() -> Vec<Service> {
    manifest().services
}

/// The services the manifest says the household reaches.
fn household_in(services: &[Service]) -> BTreeSet<&str> {
    services
        .iter()
        .filter(|service| service.bind == Some(Bind::Lan))
        .map(|service| service.id.as_str())
        .collect()
}

/// Nothing is published beyond loopback without a reason written down.
///
/// The default is the safe one and the exception is the thing that costs somebody
/// a sentence, which is the only arrangement that survives a service being added
/// by whoever is in a hurry. A new service published on every interface does not
/// need to be noticed in review: it is red until it is either bound to loopback or
/// argued for.
#[test]
fn nothing_is_published_beyond_loopback_without_a_declared_reason() {
    let household: BTreeSet<&str> = HOUSEHOLD.iter().map(|(id, _)| *id).collect();
    let beyond: Vec<String> = publications(&|_| None)
        .into_iter()
        .filter(|published| published.address != LOOPBACK)
        .filter(|published| !household.contains(published.service.as_str()))
        .map(|published| {
            format!(
                "{}: {} publishes {} on {:?}",
                published.file, published.service, published.port, published.address
            )
        })
        .collect();
    assert!(
        beyond.is_empty(),
        "these are reachable from the network and nothing says why — publish each on \
         {LOOPBACK}, or add its service to HOUSEHOLD with the reason it belongs there: \
         {beyond:?}"
    );
}

/// Every service the stack calls an admin service is published on loopback.
///
/// The other direction of the same rule, and it is not the same test: the one
/// above reads what the Compose files publish, so a service the manifest declares
/// and no file publishes at all would pass it in silence. This starts from the
/// declaration and goes looking, by host port rather than by service name —
/// the torrent client has no network namespace of its own and its port is published by
/// the tunnel it borrows one from.
#[test]
fn every_service_the_stack_calls_admin_is_published_on_loopback() {
    let published = publications(&|_| None);
    let mut wrong: Vec<String> = Vec::new();
    for service in declared() {
        let Some(port) = service.port else { continue };
        if service.bind != Some(Bind::Loopback) {
            continue;
        }
        let mut mapped = published
            .iter()
            .filter(|found| found.port == port)
            .peekable();
        assert!(
            mapped.peek().is_some(),
            "the manifest declares {} on port {port} and no Compose file publishes it",
            service.id
        );
        wrong.extend(
            mapped
                .filter(|found| found.address != LOOPBACK)
                .map(|found| format!("{}: {} on {:?}", found.file, service.id, found.address)),
        );
    }
    assert!(
        wrong.is_empty(),
        "the stack calls these admin services and publishes them where the network can \
         reach them: {wrong:?}"
    );
}

/// The household list and the stack name the same services.
///
/// The stack already records which tier each service is in, one field per service,
/// and that field is what its own repository validates its Compose files against.
/// Holding this list to it means the exceptions here are the stack's exceptions
/// rather than a second opinion about them, and that a pin move which reclassifies
/// a service is red here until somebody writes down what changed.
#[test]
fn the_household_list_names_the_services_the_stack_calls_household() {
    let services = declared();
    let declared: BTreeSet<&str> = household_in(&services);
    let listed: BTreeSet<&str> = HOUSEHOLD.iter().map(|(id, _)| *id).collect();
    assert_eq!(
        listed, declared,
        "HOUSEHOLD and the embedded manifest disagree about which services the household \
         reaches"
    );
}

/// Every household service says why it is one.
///
/// An exception list whose entries may be blank is a list of names, and a name
/// explains nothing to whoever reads it next. A reason is a sentence somebody can
/// disagree with.
#[test]
fn every_household_service_says_why_it_is_one() {
    let silent: Vec<&str> = HOUSEHOLD
        .iter()
        .filter(|(_, reason)| reason.split_whitespace().count() < 4)
        .map(|(id, _)| *id)
        .collect();
    assert!(
        silent.is_empty(),
        "these are published to the network and the list does not say what for: {silent:?}"
    );
}

/// Every household service is published on the LAN knob, and none on loopback.
///
/// The other direction from the rule above, and it is not the same claim. That one
/// says nothing is on the network without a reason; this one says the services whose
/// whole purpose is being reachable from a television actually are — a household
/// service quietly pinned to loopback is a library nobody can watch, and it would pass
/// every check on this page that only looks for things reaching too far.
///
/// Held by narrowing the knob and finding every one of them moved with it, because a
/// mapping that spelled an address out would be one the operator's own setting does
/// not reach.
#[test]
fn every_household_service_is_published_through_the_one_setting_that_narrows_them() {
    let services = declared();
    let household = household_in(&services);
    assert!(
        !household.is_empty(),
        "the manifest calls no service a household one, so this checks nothing"
    );
    let published = publications(&narrowed);
    let mut wrong: Vec<String> = Vec::new();
    for service in &services {
        let Some(port) = service.port else { continue };
        if !household.contains(service.id.as_str()) {
            continue;
        }
        let mut mapped = published
            .iter()
            .filter(|found| found.port == port)
            .peekable();
        assert!(
            mapped.peek().is_some(),
            "the manifest calls {} a household service on port {port} and no Compose file \
             publishes it",
            service.id
        );
        wrong.extend(
            mapped
                .filter(|found| found.address != NARROWED)
                .map(|found| format!("{}: {} on {:?}", found.file, service.id, found.address)),
        );
    }
    assert!(
        wrong.is_empty(),
        "the stack calls these household services and publishes them somewhere the one \
         setting that narrows the household tier does not reach: {wrong:?}"
    );
}

/// What the one setting comes to where nobody has touched it reaches the network.
///
/// The rule above says every household mapping is written through the knob. This says
/// what the knob comes to, and the two are not the same claim: a knob defaulting to
/// `127.0.0.1` would leave every household service answering this machine alone, and
/// every check on this page would stay green while the television could not reach the
/// library. Being reachable is the whole of what the tier is for, so the default is
/// the requirement rather than an implementation detail of it.
///
/// Both places the default is written are read, because either one alone would be a
/// default the other could contradict: the address each mapping is written with where
/// the operator's file names nothing, and the value the stack's own settings file
/// ships, which is what that file starts from.
#[test]
fn the_one_setting_that_narrows_the_household_tier_reaches_the_network_until_it_is_narrowed() {
    let services = declared();
    let household = household_in(&services);
    let ports: BTreeSet<u16> = services
        .iter()
        .filter(|service| household.contains(service.id.as_str()))
        .filter_map(|service| service.port)
        .collect();
    let mut defaults: Vec<(String, String)> = publications(&|_| None)
        .into_iter()
        .filter(|published| ports.contains(&published.port))
        .map(|published| {
            (
                format!("{}: {}", published.file, published.address),
                published.address,
            )
        })
        .collect();
    let example = STACK
        .get_file(SETTINGS_FILE)
        .and_then(include_dir::File::contents_utf8)
        .map(EnvFile::parse);
    let shipped = example
        .as_ref()
        .and_then(|settings| settings.get(SETTING))
        .unwrap_or_default();
    defaults.push((format!("{SETTINGS_FILE}: {SETTING}"), shipped.to_owned()));
    assert!(
        defaults.len() > household.len(),
        "fewer defaults were found than there are household services, so this is reading \
         the wrong thing: {defaults:?}"
    );
    let narrow: Vec<&(String, String)> = defaults
        .iter()
        .filter(|(_, written)| {
            written
                .parse::<IpAddr>()
                .ok()
                .is_none_or(|address| address.is_loopback())
        })
        .collect();
    assert!(
        narrow.is_empty(),
        "the household tier defaults to somewhere a television cannot reach, so the \
         services whose whole purpose is being reachable are not: {narrow:?}"
    );
}
