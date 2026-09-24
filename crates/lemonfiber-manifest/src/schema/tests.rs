use super::{ApiKind, Bind, Criticality, HealthKind, KeySource, Manifest, Protocol, Service};
use crate::Error;

const MINIMAL: &str = r#"
schema_version = 1
stack_version = "1.0.0"
min_cli_version = "0.4.0"

[[profile]]
id = "torrent"
name = "Torrents"
description = "Torrent downloading, VPN-isolated"
protocol = "torrent"

[[form]]
id = "dl"
name = "Download"
description = "You have a link — fetch it."
profiles = ["torrent"]

[[service]]
id = "qbittorrent"
name = "qBittorrent"
profile = "torrent"
image = "lscr.io/linuxserver/qbittorrent"
tag = "5.0.3"
port = 8081
bind = "loopback"
health = { kind = "http", path = "/api/v2/app/version", timeout_s = 60 }
api = { kind = "qbittorrent", key_source = "generated" }
grants = ["NET_ADMIN"]
criticality = "core"
license = "GPL-2.0-only"
upstream = "https://github.com/qbittorrent/qBittorrent"
last_release = "2026-01-09"
describes = "Downloads torrents"
without_it = "No torrent downloads"
depends_on = ["gluetun"]
"#;

/// One removal, appended to a manifest that declares the service it names as a
/// replacement — so the record points at something an operator can actually run.
const DROPPED: &str = r#"
[[removed]]
id = "readarr"
removed_in = "1.0.0"
reason = "Discontinued upstream in 2025; the project is archived and releases nothing."
replaced_by = "qbittorrent"
"#;

/// Parsed, or absent.
///
/// Assertions go through combinators rather than destructuring, because a
/// `let … else` needs an arm for the case that cannot happen, and that arm
/// is a line no test can ever reach.
fn parse(text: &str) -> Option<Manifest> {
    Manifest::from_toml(text).ok()
}

/// The first declared service, for the tests that read one.
fn service(text: &str) -> Option<Service> {
    parse(text).and_then(|manifest| manifest.services.into_iter().next())
}

#[test]
fn reads_every_declared_collection() {
    let counted = parse(MINIMAL).map(|manifest| {
        (
            manifest.profiles.len(),
            manifest.forms.len(),
            manifest.services.len(),
            manifest.stack_version,
            manifest.min_cli_version,
        )
    });
    assert_eq!(
        counted,
        Some((1, 1, 1, "1.0.0".to_owned(), "0.4.0".to_owned()))
    );
}

#[test]
fn reads_the_enumerated_fields_of_a_service() {
    let read = service(MINIMAL).map(|service| {
        (
            service.bind,
            service.criticality,
            service.port,
            service.depends_on,
        )
    });
    assert_eq!(
        read,
        Some((
            Some(Bind::Loopback),
            Criticality::Core,
            Some(8081),
            vec!["gluetun".to_owned()]
        ))
    );
}

/// A stack description written before the rename is still read.
///
/// The field was called `capabilities` and the word is now needed for something
/// else. An operator's own stack description is theirs, so the old spelling is
/// accepted rather than refused — and because the manifest refuses unknown fields,
/// dropping the alias would turn every stack written until now into a parse failure
/// rather than a warning.
#[test]
fn a_stack_written_before_the_rename_still_reads_its_kernel_grants() {
    let older = MINIMAL.replace("grants = ", "capabilities = ");
    let granted = service(&older).map(|service| service.grants);
    let current = service(MINIMAL).map(|service| service.grants);

    assert_eq!(
        granted,
        Some(vec!["NET_ADMIN".to_owned()]),
        "the old spelling reads as what it always meant"
    );
    assert_eq!(
        granted, current,
        "and reads as the same thing the new one does"
    );
}

#[test]
fn reads_the_health_probe_a_service_declares() {
    let probe = service(MINIMAL)
        .and_then(|service| service.health)
        .map(|health| (health.kind, health.timeout_s));
    assert_eq!(probe, Some((HealthKind::Http, Some(60))));
}

#[test]
fn reads_how_a_service_is_talked_to() {
    let api = service(MINIMAL)
        .and_then(|service| service.api)
        .map(|api| (api.kind, api.key_source, api.path));
    assert_eq!(
        api,
        Some((ApiKind::Qbittorrent, KeySource::Generated, None))
    );
}

#[test]
fn a_profile_declares_the_provider_it_needs() {
    let declared = parse(MINIMAL)
        .and_then(|manifest| manifest.profiles.into_iter().next())
        .map(|profile| profile.protocol);
    assert_eq!(declared, Some(Some(Protocol::Torrent)));
}

#[test]
fn a_profile_that_needs_no_provider_declares_none() {
    let text = MINIMAL.replace("protocol = \"torrent\"\n", "");
    let declared = Manifest::from_toml(&text)
        .ok()
        .and_then(|manifest| manifest.profiles.into_iter().next())
        .map(|profile| profile.protocol);
    assert_eq!(declared, Some(None));
}

#[test]
fn a_form_is_composable_unless_it_says_otherwise() {
    let composable = parse(MINIMAL)
        .and_then(|manifest| manifest.forms.into_iter().next())
        .map(|form| form.composable);
    assert_eq!(composable, Some(true));
}

#[test]
fn refuses_a_schema_generation_it_cannot_read() {
    let text = MINIMAL.replace("schema_version = 1", "schema_version = 99");
    let refusal = Manifest::from_toml(&text).err().map(|err| err.to_string());
    assert_eq!(
        refusal.as_deref(),
        Some("the manifest declares schema version 99, and this build reads [1]"),
        "the refusal names the version found and the versions supported"
    );
}

/// One profile per service is a shape rather than a rule to enforce: the field is a
/// required string, so a service naming several is unparsable and one naming none is
/// too. Worth a test of its own because it is the guarantee every closure rests on —
/// a service in two profiles would start twice, or not at all, depending on which
/// pass looked at it.
#[test]
fn a_service_declares_exactly_one_profile_and_no_other_shape_parses() {
    let several = format!("{MINIMAL}\n[[service]]\nprofiles = [\"torrent\", \"usenet\"]\n");
    assert!(
        Manifest::from_toml(&several).is_err(),
        "a service naming several profiles must not parse"
    );

    let none = MINIMAL.replace("profile = \"torrent\"", "");
    assert!(
        Manifest::from_toml(&none).is_err(),
        "a service naming no profile must not parse either"
    );
}

#[test]
fn refuses_a_field_it_does_not_know() {
    let text = format!("{MINIMAL}\n[[service]]\nunknown_field = true\n");
    let refusal = Manifest::from_toml(&text)
        .err()
        .map(|err| matches!(err, Error::Syntax(_)));
    assert_eq!(refusal, Some(true));
}

#[test]
fn a_newer_generation_is_named_as_such_even_when_it_carries_unknown_fields() {
    // A future manifest declares a newer generation and, plausibly, fields
    // this build has never heard of. The version gate must speak first — "you
    // need a newer lemonfiber" — rather than the parser rejecting a field.
    let text = format!(
        "{}\n[[service]]\nfield_from_the_future = true\n",
        MINIMAL.replace("schema_version = 1", "schema_version = 99")
    );
    assert!(matches!(
        Manifest::from_toml(&text),
        Err(Error::UnsupportedSchema { found: 99, .. })
    ));
}

#[test]
fn names_every_unrecognised_declaration_in_one_pass() {
    let text = MINIMAL
        .replace(r#"kind = "qbittorrent""#, r#"kind = "plex""#)
        .replace(r#"criticality = "core""#, r#"criticality = "vital""#);
    let refusal = Manifest::from_toml(&text)
        .err()
        .map(|refused| refused.to_string())
        .unwrap_or_default();
    assert!(refusal.contains("plex"), "names the first: {refusal}");
    assert!(refusal.contains("vital"), "names the second too: {refusal}");
}

/// A stack that has dropped something says what it was and what became of it.
///
/// Read back whole rather than counted, because the count is the half that would
/// still pass if every field arrived empty.
#[test]
fn reads_what_a_stack_has_dropped_and_what_took_its_place() {
    let text = format!("{MINIMAL}{DROPPED}");
    let recorded = parse(&text)
        .and_then(|manifest| manifest.removed.into_iter().next())
        .map(|removed| {
            (
                removed.id,
                removed.removed_in,
                removed.reason.contains("Discontinued"),
                removed.replaced_by,
            )
        });
    assert_eq!(
        recorded,
        Some((
            "readarr".to_owned(),
            "1.0.0".to_owned(),
            true,
            Some("qbittorrent".to_owned())
        ))
    );
}

/// Nothing replaced it is an answer, and the one a record has to be able to give
/// — a stack forced to name a successor would name the nearest thing to hand.
#[test]
fn a_removal_with_nothing_in_its_place_records_that_rather_than_inventing_one() {
    let text = format!(
        "{MINIMAL}{}",
        DROPPED.replace("replaced_by = \"qbittorrent\"\n", "")
    );
    let replaced = parse(&text)
        .and_then(|manifest| manifest.removed.into_iter().next())
        .map(|removed| removed.replaced_by);
    assert_eq!(replaced, Some(None));
}

/// The table is optional, which is what keeps the two repositories from having to
/// land together: a stack that has dropped nothing declares nothing and reads as
/// having dropped nothing, rather than as a manifest missing a table.
///
/// The generation does not move for it, and this is the assertion that pins that
/// decision to something executable — a stack recording a removal reads under the
/// generation every stack already declares, so neither repository is waiting on the
/// other to renumber before it can land.
#[test]
fn a_stack_that_has_dropped_nothing_reads_as_having_dropped_nothing() {
    let recorded = parse(&format!("{MINIMAL}{DROPPED}")).map(|manifest| manifest.removed.len());
    let silent = parse(MINIMAL).map(|manifest| manifest.removed.len());

    assert_eq!(silent, Some(0));
    assert_eq!(
        recorded,
        Some(1),
        "and the same generation reads one that did"
    );
}

/// A field nobody declared on a removal is refused, the way it is everywhere else
/// here: a misspelled `replaced_by` that is quietly dropped records a removal with
/// no replacement, which is a different fact from the one somebody wrote.
#[test]
fn a_removal_declaring_a_field_this_build_does_not_know_is_refused() {
    let text = format!(
        "{MINIMAL}{}",
        DROPPED.replace("replaced_by = ", "replaced_with = ")
    );
    assert!(Manifest::from_toml(&text).is_err());
}

#[test]
fn parses_the_stack_this_binary_embeds() {
    let embedded = include_str!("../../../../assets/media-stack/stack.toml");
    let counted = parse(embedded).map(|manifest| {
        (
            manifest.profiles.len(),
            manifest.forms.len(),
            manifest.services.len(),
        )
    });
    assert_eq!(counted, Some((12, 11, 20)));
}
