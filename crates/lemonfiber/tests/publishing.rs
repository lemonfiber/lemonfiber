//! Which interface the stack this binary carries publishes each port on.
//!
//! Read from the embedded copy — the same `STACK` the binary writes to an
//! operator's disk — rather than from the checked-out submodule. The embedded copy
//! is the artifact: those are the bytes a machine ends up running, and the
//! directory beside them belongs to another repository that a clone need not have
//! populated at all. The bytes are fixed into this test binary when it is
//! compiled, and `build.rs` refuses to produce a build without them, so there is
//! no arrangement in which this reads an empty directory and finds nothing to
//! object to.
//!
//! The stack's own repository checks this too, against the same manifest. It
//! checks it on its own pull requests, which is not the moment a stack arrives
//! here — moving the submodule pin brings in a whole stack and runs none of that
//! repository's gates. This one runs wherever this binary is built.

use std::collections::BTreeSet;
use std::net::IpAddr;

use include_dir::Dir;
use lemonfiber::cli::STACK;
use lemonfiber_core::config::env::EnvFile;
use lemonfiber_manifest::{Bind, Manifest, Service};

/// The address a service reachable only from the host machine is published on.
const LOOPBACK: &str = "127.0.0.1";

/// The one setting that decides which interface the household tier is published on.
///
/// A knob rather than an address, because the stack has no way to learn which of a
/// host's addresses is the LAN one: that varies by machine, changes with DHCP, and is
/// different again on a laptop that moves. So the default is every interface and the
/// operator is told so, which is the honest half of the trade — and this is the word
/// every household mapping has to be written through, so narrowing it narrows all of
/// them at once.
const LAN_BIND: &str = "${LAN_BIND";

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
        "jellyfin",
        "a television plays the library, and a television is not the host",
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
        "caddy",
        "the hostnames and certificates every other household service is reached by",
    ),
];

/// One published port: which service publishes it, on what address, and where
/// that was written.
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

/// Every port the embedded stack publishes.
fn publications() -> Vec<Publication> {
    let mut yaml = Vec::new();
    collect(&STACK, &mut yaml);
    let mut found = Vec::new();
    for (file, text) in yaml {
        found.append(&mut published_in(&file, &text));
    }
    assert!(
        !found.is_empty(),
        "the embedded stack publishes no ports at all, which means this is reading the wrong thing"
    );
    found
}

/// Every YAML file in the embedded stack, with its path.
fn collect(dir: &Dir<'_>, into: &mut Vec<(String, String)>) {
    for file in dir.files() {
        let path = file.path();
        let yaml = path.extension().is_some_and(|kind| {
            kind.eq_ignore_ascii_case("yml") || kind.eq_ignore_ascii_case("yaml")
        });
        if !yaml {
            continue;
        }
        if let Some(text) = file.contents_utf8() {
            into.push((path.to_string_lossy().replace('\\', "/"), text.to_owned()));
        }
    }
    for child in dir.dirs() {
        collect(child, into);
    }
}

/// The ports one Compose file publishes.
///
/// Read as text rather than as YAML, because what is being checked is a string an
/// author typed — `127.0.0.1:8989:8989` — and a parser would hand back the same
/// string having also brought in a dependency. Commented-out lines are skipped:
/// a mapping nobody applies is not a mapping.
fn published_in(file: &str, text: &str) -> Vec<Publication> {
    let mut found = Vec::new();
    let mut service = String::new();
    let mut listing = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - trimmed.len();
        if indent == 0 {
            service.clear();
            listing = false;
            continue;
        }
        if indent == 2 && trimmed.ends_with(':') {
            trimmed.trim_end_matches(':').clone_into(&mut service);
            listing = false;
            continue;
        }
        listing = trimmed.starts_with("ports:") || (listing && trimmed.starts_with('-'));
        if !listing {
            continue;
        }
        found.extend(mappings(trimmed).map(|(port, address)| Publication {
            service: service.clone(),
            port,
            address,
            file: file.to_owned(),
        }));
    }
    found
}

/// The port mappings written on one line of a `ports:` entry.
///
/// Every double-quoted run on the line, which is how a mapping is always written
/// here — the surrounding YAML is flow or block sequence syntax either way.
fn mappings(line: &str) -> impl Iterator<Item = (u16, String)> + '_ {
    line.split('"').skip(1).step_by(2).filter_map(mapping)
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

/// The address a mapping written through the knob falls back to.
///
/// `${LAN_BIND:-0.0.0.0}` is Compose's own form for *this unless the operator's file
/// says otherwise*, and what follows `:-` is the unless. A mapping written through
/// the knob with no fallback at all has none: Compose expands an unset variable to
/// nothing, which happens to publish on every interface, and a default arrived at by
/// an expansion nobody wrote down is not one anybody argued for.
fn falls_back_to(address: &str) -> Option<&str> {
    address
        .strip_prefix(LAN_BIND)?
        .strip_suffix('}')?
        .strip_prefix(":-")
}

/// The manifest the embedded stack declares itself with.
fn declared() -> Vec<Service> {
    let Some(file) = STACK.get_file("stack.toml") else {
        unreachable!("the build refuses to produce this binary without the embedded manifest")
    };
    let Some(text) = file.contents_utf8() else {
        unreachable!("the embedded manifest is the text the build has already parsed")
    };
    let Ok(manifest) = Manifest::from_toml(text) else {
        unreachable!("the build refuses to produce this binary against a manifest it cannot read")
    };
    manifest.services
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
    let beyond: Vec<String> = publications()
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
/// qBittorrent has no network namespace of its own and its port is published by
/// the tunnel it borrows one from.
#[test]
fn every_service_the_stack_calls_admin_is_published_on_loopback() {
    let published = publications();
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
/// Held through the knob rather than through the address it defaults to, because what
/// makes this configurable is that every one of them is written the same way: a
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
    let published = publications();
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
                .filter(|found| !found.address.starts_with(LAN_BIND))
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
/// default the other could contradict: the fallback inside each mapping, which is what
/// applies where the operator's file names nothing, and the value the stack's own
/// settings file ships, which is what that file starts from.
#[test]
fn the_one_setting_that_narrows_the_household_tier_reaches_the_network_until_it_is_narrowed() {
    let services = declared();
    let household = household_in(&services);
    let ports: BTreeSet<u16> = services
        .iter()
        .filter(|service| household.contains(service.id.as_str()))
        .filter_map(|service| service.port)
        .collect();
    let mut defaults: Vec<(String, String)> = publications()
        .into_iter()
        .filter(|published| ports.contains(&published.port))
        .map(|published| {
            let written = falls_back_to(&published.address).unwrap_or_default();
            (
                format!("{}: {}", published.file, published.address),
                written.to_owned(),
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
