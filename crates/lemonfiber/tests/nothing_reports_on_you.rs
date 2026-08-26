//! What this product knows about the person running it, and what would notice it
//! starting to know something.
//!
//! Every claim here is a claim about an absence, which is the hardest kind to keep:
//! it is true today by nobody having added anything, and one commit undoes it. So
//! none of these tests asserts the absence directly. Each reads a corpus that grows
//! when somebody adds something — the resolved dependency graph, the hosts the
//! shipped source names, the reaches of the one port that produces unpredictable
//! bytes — and fails on anything in it that is not written down here with a reason.
//!
//! **The corpus is what ships**, in the sense [`source_tree::shipped`] means: a
//! file's own tests cut off the bottom, and a module the compiler only builds for
//! tests dropped whole. A fixture naming an address is a fixture.
//!
//! **What is not covered**, since a guard that reads as wider than it is becomes a
//! reason not to look. A host assembled at runtime rather than written down is
//! invisible to the sweep below, and so is one that arrives from configuration —
//! the operator's own indexer is exactly that, and is theirs to name. What the
//! randomness sweep holds is the *reach*, not what becomes of the bytes: the
//! declaration is where somebody has to write that down, and writing it down is the
//! moment anybody asks whether the value outlives the run. Neither gap is closed by
//! reading source text, and both are closed at the seam instead — there is one
//! transport and one randomness port, and a new reach of either is red here before
//! it is anything else.

use std::collections::{BTreeMap, BTreeSet};

mod source_tree;

use source_tree::{shipped, workspace_root};

/// Package names that would mean this product had started collecting.
///
/// Stems rather than exact names: a family of crates is published under one
/// prefix — an exporter, a core, an integration — and banning the one somebody
/// happens to reach for first would leave the rest. A package matches a stem when
/// its name is the stem, or begins with the stem and a separator.
///
/// This is not a guess at what is out there. It is the list `deny.toml` carries, so
/// the two cannot drift, and the test below holds them to each other.
const COLLECTORS: &[&str] = &[
    "amplitude",
    "aptabase",
    "bugsnag",
    "countly",
    "datadog",
    "google-analytics",
    "libhoney",
    "mixpanel",
    "opentelemetry",
    "posthog",
    "rollbar",
    "segment",
    "sentry",
    "snowplow",
];

/// Whether lemonfiber itself sends a request to a host it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
    /// lemonfiber sends a request there, or asks a container to.
    Asked,
    /// The name appears in something an operator reads and nothing fetches it.
    Printed,
}

/// Every host outside this machine that the shipped half of this workspace names,
/// what lemonfiber does about it, and why it is there.
///
/// The list is short by design and the shortness is the claim. A host arriving here
/// is a decision somebody writes a sentence for; a host arriving in the source and
/// not here fails, which is the half that survives an update check being added by
/// whoever is in a hurry.
const NAMED: &[(&str, Reach, &str)] = &[
    (
        "docs.docker.com",
        Reach::Printed,
        "where a remedy sends an operator who has no container engine; the sentence is printed \
         beside the finding and nothing here fetches it",
    ),
    (
        "docs.lemonfiber.app",
        Reach::Printed,
        "where the reference for an error code lives, printed beside the code so somebody \
         reading a failure can go and look it up themselves",
    ),
    (
        "github.com",
        Reach::Asked,
        "the community quality guides Recyclarr syncs profiles from, asked for once per \
         diagnosis so a sync that would bring nothing back is reported rather than assumed \
         fine",
    ),
    (
        "icanhazip.com",
        Reach::Asked,
        "the second of two independent addresses the leak check compares egress against, \
         asked by the container rather than by lemonfiber, because two sources disagreeing is \
         the only way a wrong one is distinguishable from a working tunnel",
    ),
    (
        "ifconfig.me",
        Reach::Asked,
        "the first of those two, and the default an operator replaces or switches off with \
         one setting",
    ),
    (
        "trash-guides.info",
        Reach::Printed,
        "named in a comment as where the community profiles come from; Recyclarr syncs them \
         on its own schedule and lemonfiber never reads that site",
    ),
];

/// The reaches of the one port that produces unpredictable bytes, and what each
/// makes of them.
///
/// The question a persistent installation identifier would be the wrong answer to
/// is *what is this value, and does it outlive the run* — so that is what an entry
/// says. Three, and none of them is about the installation: two are credentials for
/// a service on this machine, and the third is a per-bundle salt that exists so a
/// redacted value cannot be recognised across two bundles, which is the opposite of
/// an identifier.
const MINTING: &[(&str, &str)] = &[
    (
        "crates/lemonfiber-api/src/guard.rs",
        "the token every web request must carry, and the name a long job is redeemed by. Minted \
         per run, held in memory, and never written down — a second run of the surface issues \
         another",
    ),
    (
        "crates/lemonfiber-core/src/secret.rs",
        "the two passwords lemonfiber has to mint rather than read: qBittorrent's web UI and \
         Jellyfin's administrator. Recorded in the settings file because the services \
         authenticate with them, and sent to those services on this machine and nowhere else",
    ),
    (
        "crates/lemonfiber-core/src/bundle/allowed.rs",
        "the salt a support bundle's redaction marks are derived from. Made for one bundle and \
         discarded with it, so the same withheld value carries a different mark in the next \
         one — deliberately not stable, and stability is what an identifier is",
    ),
];

/// What a file that takes bytes from the randomness port holds: the port's name,
/// and the call.
///
/// Both, because either alone reads the wrong files. `.bytes(` on its own is also
/// how a string is walked byte by byte, which two files here do and neither has
/// anything to do with randomness; the port's name on its own is also what a
/// signature passing the port along says, and a file that only hands it on mints
/// nothing.
const MINTED_BY: (&str, &str) = ("Random", ".bytes(");

/// The one adapter allowed to reach the operating system's randomness, and the call
/// it reaches it with.
const OS_RANDOMNESS: (&str, &str) = ("crates/lemonfiber-core/src/adapters/random.rs", "getrandom");

/// A package this workspace is known to depend on, so a reader of `Cargo.lock` that
/// found nothing is told apart from a graph that holds nothing.
const KNOWN_DEPENDENCY: &str = "reqwest";

/// Whether a package name is one of the stems.
fn collects(name: &str) -> bool {
    COLLECTORS.iter().any(|stem| {
        name == *stem
            || name
                .strip_prefix(stem)
                .is_some_and(|rest| rest.starts_with('-') || rest.starts_with('_'))
    })
}

/// Every package in the resolved dependency graph.
fn resolved() -> BTreeSet<String> {
    let root = workspace_root();
    let Ok(text) = std::fs::read_to_string(root.join("Cargo.lock")) else {
        unreachable!("the workspace this test is compiled from has a lock file")
    };
    let Ok(lock) = text.parse::<toml::Table>() else {
        unreachable!("the lock file cargo writes is TOML")
    };
    let found: BTreeSet<String> = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .map(|packages| {
            packages
                .iter()
                .filter_map(|package| package.get("name"))
                .filter_map(toml::Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    assert!(
        found.contains(KNOWN_DEPENDENCY),
        "the graph does not hold {KNOWN_DEPENDENCY}, which means this is reading the wrong \
         file — every claim below it would be a claim about nothing"
    );
    found
}

/// The names cargo-deny is told to refuse.
fn banned() -> BTreeSet<String> {
    let root = workspace_root();
    let Ok(text) = std::fs::read_to_string(root.join("deny.toml")) else {
        unreachable!("the workspace this test is compiled from configures cargo-deny")
    };
    let Ok(config) = text.parse::<toml::Table>() else {
        unreachable!("cargo-deny's configuration is TOML")
    };
    config
        .get("bans")
        .and_then(|bans| bans.get("deny"))
        .and_then(toml::Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.get("name").or(Some(entry)))
                .filter_map(toml::Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// The text inside double quotes on one line.
fn quoted(line: &str) -> impl Iterator<Item = &str> {
    line.split('"').skip(1).step_by(2)
}

/// The host a piece of text names, where it names one outside this machine.
///
/// A name with no dot is a container on the stack's own network or a placeholder a
/// format string fills in; a loopback address is this machine; and the top-level
/// names reserved for documentation cannot be registered by anybody, so a URL under
/// one is an illustration rather than a destination.
fn host_of(said: &str) -> Option<String> {
    let after = said
        .split_once("://")
        .filter(|(scheme, _)| scheme.ends_with("http") || scheme.ends_with("https"))
        .map(|(_, rest)| rest)?;
    let host: String = after
        .chars()
        .take_while(|letter| letter.is_ascii_alphanumeric() || *letter == '.' || *letter == '-')
        .collect();
    let reserved = [".example", ".test", ".invalid", ".localhost"];
    if !host.contains('.')
        || host.parse::<std::net::IpAddr>().is_ok()
        || reserved.iter().any(|end| host.ends_with(end))
    {
        return None;
    }
    Some(host.to_ascii_lowercase())
}

/// Every host outside this machine the shipped half of this workspace names, and
/// where each was written.
fn hosts() -> BTreeMap<String, Vec<String>> {
    let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (file, ships) in shipped() {
        for (number, line) in ships.lines().enumerate() {
            for host in quoted(line)
                .chain(std::iter::once(line))
                .filter_map(host_of)
            {
                let at = format!("{file}:{}", number + 1);
                let seen = found.entry(host).or_default();
                if !seen.contains(&at) {
                    seen.push(at);
                }
            }
        }
    }
    found
}

/// Nothing in the dependency graph is a thing that reports on its users.
///
/// The graph rather than the manifests, because a collector arrives as somebody
/// else's dependency at least as easily as it arrives as one of ours — and a
/// direct-dependency check would pass on a transitive one while reading as though
/// it had looked.
#[test]
fn nothing_this_binary_is_built_from_collects() {
    let carried: Vec<String> = resolved()
        .into_iter()
        .filter(|name| collects(name))
        .collect();
    assert!(
        carried.is_empty(),
        "these are in the resolved graph and each of them exists to report somewhere: \
         {carried:?}"
    );
}

/// The ban list and the list above name the same things.
///
/// Two gates over one claim, and they answer different questions: this test reads
/// the graph as resolved and matches a family by its stem, while `cargo deny` reads
/// a name at a time and is the one that runs on a dependency bump nobody opened.
/// Neither is redundant, and a stem on one and not the other is a stem nothing
/// enforces on the path it was written for.
#[test]
fn what_the_dependency_gate_refuses_is_what_this_names() {
    let listed: BTreeSet<String> = COLLECTORS.iter().map(|stem| (*stem).to_owned()).collect();
    assert_eq!(
        banned(),
        listed,
        "the ban list cargo-deny reads and the stems this test sweeps for disagree — one of \
         them is enforcing something the other is not"
    );
}

/// Every host the shipped half names is written down, with what it is for.
///
/// Held in both directions. A host in the source and not on the list fails, which is
/// what makes an update check somebody adds a red run rather than a thing to notice
/// in review; and a host on the list that the source no longer names fails too,
/// because an entry excepting nothing reads as a decision about this product and is
/// a decision about a product that has moved on.
#[test]
fn every_host_beyond_this_machine_is_written_down() {
    let found = hosts();
    assert!(
        found.contains_key("ifconfig.me"),
        "the sweep found no host it is known to ship, so it is reading the wrong tree: {:?}",
        found.keys().collect::<Vec<_>>()
    );
    let declared: BTreeSet<&str> = NAMED.iter().map(|(host, _, _)| *host).collect();
    let undeclared: Vec<String> = found
        .iter()
        .filter(|(host, _)| !declared.contains(host.as_str()))
        .map(|(host, at)| format!("{host} ({})", at.join(", ")))
        .collect();
    assert!(
        undeclared.is_empty(),
        "the shipped source names these and nothing says what they are for — say whether \
         lemonfiber asks them for anything, and what turning that off would cost: {undeclared:?}"
    );
    let stale: Vec<&str> = declared
        .into_iter()
        .filter(|host| !found.contains_key(*host))
        .collect();
    assert!(
        stale.is_empty(),
        "these are written down as hosts this product names and nothing names them any \
         more: {stale:?}"
    );
}

/// Every host written down says what it is for.
///
/// A list whose entries may be blank is a list of names, and a name explains nothing
/// to whoever reads it next. A reason is a sentence somebody can disagree with.
#[test]
fn every_host_written_down_says_what_it_is_for() {
    let silent: Vec<&str> = NAMED
        .iter()
        .filter(|(_, _, reason)| reason.split_whitespace().count() < 8)
        .map(|(host, _, _)| *host)
        .collect();
    assert!(
        silent.is_empty(),
        "these are named in what ships and the list does not say what for: {silent:?}"
    );
    assert!(
        NAMED.iter().any(|(_, reach, _)| *reach == Reach::Asked),
        "nothing on this list is asked for anything, which means the distinction it draws is \
         not being drawn"
    );
}

/// The hosts this product asks for something are exactly the ones it tells the
/// operator about.
///
/// The sweep above reads source text and the enumeration is what an operator can
/// list, and until they are held to each other neither is worth much: a host could
/// be written down here and left out of the list somebody reads, or listed and
/// reached from nowhere. Held with the settings as they arrive and no stack
/// materialised, which is what leaves exactly the destinations that are constants —
/// the registries come from whichever images a stack declares, and the indexer and
/// the provider are wherever this operator pointed them.
#[test]
fn what_the_source_asks_for_is_what_the_operator_is_told_about() {
    let asked: BTreeSet<&str> = NAMED
        .iter()
        .filter(|(_, reach, _)| *reach == Reach::Asked)
        .map(|(host, _, _)| *host)
        .collect();
    let listed: BTreeSet<String> =
        lemonfiber_core::outbound::leaving(&lemonfiber_core::config::Settings::default(), &[])
            .ours
            .into_iter()
            .flat_map(|entry| entry.destination)
            .filter_map(|where_to| host_of(&where_to))
            .collect();
    assert!(
        !listed.is_empty(),
        "the enumeration names nowhere at all, so this is holding the sweep to nothing"
    );
    assert_eq!(
        listed,
        asked.iter().map(|host| (*host).to_owned()).collect(),
        "the shipped source asks somewhere the operator is not told about, or the list \
         names somewhere nothing asks — the enumeration is what an operator acts on, so \
         the two cannot differ"
    );
}

/// Nothing mints a value this installation could be recognised by.
///
/// There is one source of unpredictable bytes in this workspace and it is behind a
/// port, which is what makes the question answerable at all: every value that could
/// become an identifier passes through one call, and this holds every reach of that
/// call to an entry saying what the bytes become and whether they outlive the run.
///
/// A reach arriving without an entry is red. That is the whole mechanism — nothing
/// here can tell an identifier from a password by looking at it, and pretending
/// otherwise would be a guard that reads as stronger than it is.
#[test]
fn nothing_mints_a_value_this_installation_would_be_known_by() {
    let (port, call) = MINTED_BY;
    let reaching: BTreeSet<String> = shipped()
        .into_iter()
        .filter(|(_, ships)| ships.contains(port) && ships.contains(call))
        .map(|(file, _)| file)
        .collect();
    assert!(
        !reaching.is_empty(),
        "nothing in this workspace takes bytes from the randomness port, which means this is \
         looking for the wrong call and would pass over anything that did"
    );
    let declared: BTreeSet<String> = MINTING.iter().map(|(file, _)| (*file).to_owned()).collect();
    assert_eq!(
        reaching, declared,
        "the shipped half of these takes unpredictable bytes and this list disagrees about \
         which may — say what the bytes become, and say it having checked that the value does \
         not identify the installation across requests"
    );
}

/// One adapter reaches the operating system's randomness, and nothing else does.
///
/// The port is only a seam while it is the only way through. A file taking bytes
/// from the platform directly would be outside the sweep above entirely, and the
/// failure would be silence.
#[test]
fn one_file_reaches_the_platform_for_randomness() {
    let (allowed, call) = OS_RANDOMNESS;
    let reaching: Vec<String> = shipped()
        .into_iter()
        .filter(|(_, ships)| ships.contains(call))
        .map(|(file, _)| file)
        .collect();
    assert_eq!(
        reaching,
        vec![allowed.to_owned()],
        "the platform's randomness is reached from somewhere other than the one adapter, or \
         from nowhere at all — either way the port below is no longer the only way through"
    );
}

/// Every reach of the randomness says what it makes.
#[test]
fn every_reach_of_the_randomness_says_what_it_makes() {
    let silent: Vec<&str> = MINTING
        .iter()
        .filter(|(_, becomes)| becomes.split_whitespace().count() < 8)
        .map(|(file, _)| *file)
        .collect();
    assert!(
        silent.is_empty(),
        "these mint a value and the list does not say what it is: {silent:?}"
    );
}
