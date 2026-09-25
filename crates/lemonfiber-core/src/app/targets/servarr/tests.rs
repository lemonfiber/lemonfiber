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
