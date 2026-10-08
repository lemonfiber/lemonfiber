use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::{assemble, read};
use crate::{Failure, Manifest, Violation};

const ROOT: &str = r#"schema_version = 1
stack_version = "1.0.0"
min_cli_version = "0.4.0"
include = ["services/qbittorrent.toml", "services/gluetun.toml"]

[[profile]]
id = "torrent"
name = "Torrents"
description = "Torrent downloading, VPN-isolated"
protocol = "torrent"
"#;

/// A whole service, in the file it is named for.
fn service(id: &str) -> String {
    format!(
        r#"[[service]]
id = "{id}"
name = "{id}"
profile = "torrent"
image = "lscr.io/linuxserver/{id}"
tag = "5.0.3"
port = 8081
bind = "loopback"
health = {{ kind = "http", path = "/", timeout_s = 60 }}
criticality = "core"
license = "GPL-2.0-only"
upstream = "https://github.com/example/{id}"
last_release = "2026-01-09"
describes = "Does a thing"
without_it = "The thing is not done"

[service.api]
kind = "qbittorrent"
key_source = "generated"
"#
    )
}

/// Both services the root includes, each in its own file.
fn both() -> BTreeMap<String, String> {
    files(&[
        ("qbittorrent.toml", &service("qbittorrent")),
        ("gluetun.toml", &service("gluetun")),
    ])
}

fn files(each: &[(&str, &str)]) -> BTreeMap<String, String> {
    each.iter()
        .map(|(name, text)| ((*name).to_owned(), (*text).to_owned()))
        .collect()
}

/// Every fault the files were refused with, or none.
fn faults(root: &str, services: &BTreeMap<String, String>) -> Vec<Violation> {
    match assemble(root, services) {
        Err(Failure::Assembly(found)) => found,
        _ => Vec::new(),
    }
}

/// Each fault as one line, for asserting on what was said.
fn said(found: &[Violation]) -> Vec<String> {
    found.iter().map(ToString::to_string).collect()
}

#[test]
fn the_files_read_as_one_manifest_with_its_services_in_include_order() {
    let manifest = assemble(ROOT, &both())
        .ok()
        .and_then(|text| Manifest::from_toml(&text).ok());
    let ids: Vec<String> = manifest
        .iter()
        .flat_map(|one| one.services.iter().map(|service| service.id.clone()))
        .collect();
    assert_eq!(ids, ["qbittorrent", "gluetun"]);
    assert_eq!(
        manifest.map(|one| one.include).unwrap_or_default(),
        ["services/qbittorrent.toml", "services/gluetun.toml"]
    );
}

#[test]
fn a_service_s_own_tables_stay_with_it_when_the_files_are_joined() {
    let manifest = assemble(ROOT, &both())
        .ok()
        .and_then(|text| Manifest::from_toml(&text).ok());
    let images: Vec<String> = manifest
        .iter()
        .flat_map(|one| one.services.iter().map(|service| service.image.clone()))
        .collect();
    assert_eq!(
        images,
        [
            "lscr.io/linuxserver/qbittorrent",
            "lscr.io/linuxserver/gluetun"
        ]
    );
}

#[test]
fn the_root_s_lines_keep_their_numbers_in_the_joined_text() {
    let joined = assemble(ROOT, &both()).unwrap_or_default();
    assert!(joined.starts_with(ROOT), "got: {joined}");
    assert!(
        joined.contains("\n# services/qbittorrent.toml\n"),
        "got: {joined}"
    );
}

#[test]
fn a_root_with_no_include_and_no_service_files_is_a_stack_of_no_services() {
    let root = ROOT.replace(
        "include = [\"services/qbittorrent.toml\", \"services/gluetun.toml\"]\n",
        "",
    );
    assert_eq!(assemble(&root, &BTreeMap::new()).ok(), Some(root));
}

#[test]
fn a_root_that_is_not_toml_is_a_syntax_error() {
    assert!(matches!(
        assemble("= not toml", &both()),
        Err(Failure::Syntax(_))
    ));
}

#[test]
fn a_service_in_the_root_is_named_with_the_file_it_belongs_in() {
    let root = format!("{ROOT}\n{}", service("sonarr"));
    assert_eq!(
        said(&faults(&root, &both())),
        ["stack.toml: declares service sonarr, which belongs in services/sonarr.toml"]
    );
}

#[test]
fn a_service_in_the_root_with_no_id_is_still_refused() {
    let root = format!("service = {{ name = \"x\" }}\n{ROOT}");
    assert_eq!(
        said(&faults(&root, &both())),
        ["stack.toml: declares a [[service]], and each service is in a file of its own"]
    );
}

#[test]
fn an_include_that_is_not_a_list_is_refused() {
    let root = ROOT.replace(
        "include = [\"services/qbittorrent.toml\", \"services/gluetun.toml\"]",
        "include = \"services\"",
    );
    let found = said(&faults(&root, &both()));
    assert!(
        found.contains(&"stack.toml: include is a list of the service files".to_owned()),
        "got: {found:?}"
    );
}

#[test]
fn an_entry_that_is_not_a_path_is_refused() {
    let root = ROOT.replace(
        "\"services/gluetun.toml\"]",
        "\"services/gluetun.toml\", 7]",
    );
    assert_eq!(
        said(&faults(&root, &both())),
        ["stack.toml: include holds 7, which is not a path"]
    );
}

#[test]
fn every_entry_not_of_the_form_is_named() {
    for entry in [
        "other/qbittorrent.toml",
        "services/qbittorrent.yaml",
        "services/../qbittorrent.toml",
        "services/a/qbittorrent.toml",
        "services/.toml",
        "services/.hidden.toml",
        "servicesqbittorrent.toml",
        "services\\\\qbittorrent.toml",
    ] {
        let root = ROOT.replace("services/qbittorrent.toml\"", &format!("{entry}\""));
        let found = said(&faults(&root, &both()));
        let written = entry.replace("\\\\", "\\");
        assert!(
            found.contains(&format!(
                "include entry {written}: is not of the form services/<id>.toml"
            )),
            "{entry}: {found:?}"
        );
    }
}

#[test]
fn an_entry_that_appears_twice_is_named_once_for_each_repeat() {
    let root = ROOT.replace(
        "\"services/gluetun.toml\"]",
        "\"services/gluetun.toml\", \"services/gluetun.toml\"]",
    );
    assert_eq!(
        said(&faults(&root, &both())),
        ["include entry services/gluetun.toml: appears more than once"]
    );
}

#[test]
fn an_entry_that_names_no_file_is_named() {
    let services = files(&[("qbittorrent.toml", &service("qbittorrent"))]);
    assert_eq!(
        said(&faults(ROOT, &services)),
        ["include entry services/gluetun.toml: names no file"]
    );
}

#[test]
fn a_file_no_entry_names_is_named_and_not_read() {
    let mut services = both();
    services.insert("sonarr.toml".to_owned(), "not toml at all =".to_owned());
    assert_eq!(
        said(&faults(ROOT, &services)),
        ["services/sonarr.toml: is in services/ and no include entry names it"]
    );
}

#[test]
fn a_service_file_that_is_not_toml_is_named_at_its_own_line() {
    let mut services = both();
    services.insert(
        "gluetun.toml".to_owned(),
        "[[service]]\nid = \"gluetun\"\nname = \n".to_owned(),
    );
    let found = said(&faults(ROOT, &services));
    assert_eq!(found.len(), 1, "got: {found:?}");
    assert!(
        found
            .iter()
            .all(|one| one.starts_with("services/gluetun.toml: line 3: ")),
        "got: {found:?}"
    );
}

#[test]
fn a_service_file_holding_anything_else_names_what_it_holds() {
    let mut services = both();
    services.insert(
        "gluetun.toml".to_owned(),
        format!("{}\n[[profile]]\nid = \"x\"\n", service("gluetun")),
    );
    assert_eq!(
        said(&faults(ROOT, &services)),
        ["services/gluetun.toml: holds profile, and a service file holds only its [[service]]"]
    );
}

#[test]
fn a_service_file_holding_no_service_is_named() {
    let mut services = both();
    services.insert("gluetun.toml".to_owned(), String::new());
    assert_eq!(
        said(&faults(ROOT, &services)),
        ["services/gluetun.toml: holds no [[service]]"]
    );
}

#[test]
fn a_service_file_holding_two_is_named() {
    let mut services = both();
    services.insert(
        "gluetun.toml".to_owned(),
        format!("{}\n{}", service("gluetun"), service("vpn")),
    );
    assert_eq!(
        said(&faults(ROOT, &services)),
        ["services/gluetun.toml: holds 2 [[service]], and a service file holds one"]
    );
}

#[test]
fn a_service_whose_id_is_not_its_file_s_name_is_named() {
    let mut services = both();
    services.insert("gluetun.toml".to_owned(), service("vpn"));
    assert_eq!(
        said(&faults(ROOT, &services)),
        ["services/gluetun.toml: holds service vpn, and its name says gluetun"]
    );
}

#[test]
fn a_service_with_no_id_is_named() {
    let mut services = both();
    services.insert(
        "gluetun.toml".to_owned(),
        service("gluetun").replace("id = \"gluetun\"\n", ""),
    );
    assert_eq!(
        said(&faults(ROOT, &services)),
        ["services/gluetun.toml: holds service with no id, and its name says gluetun"]
    );
}

#[test]
fn a_service_of_the_wrong_shape_is_named_at_its_own_line() {
    let mut services = both();
    services.insert(
        "gluetun.toml".to_owned(),
        service("gluetun").replace("port = 8081", "port = \"high\""),
    );
    let found = said(&faults(ROOT, &services));
    assert_eq!(found.len(), 1, "got: {found:?}");
    assert!(
        found
            .iter()
            .all(|one| one.starts_with("services/gluetun.toml: line 7: ")),
        "got: {found:?}"
    );
}

#[test]
fn every_fault_is_reported_in_one_pass() {
    let root = format!(
        "{}\n{}",
        ROOT.replace(
            "\"services/gluetun.toml\"]",
            "\"services/gluetun.toml\", \"x.toml\"]"
        ),
        service("sonarr")
    );
    let mut services = files(&[("qbittorrent.toml", &service("vpn"))]);
    services.insert("stray.toml".to_owned(), String::new());
    assert_eq!(
        said(&faults(&root, &services)),
        [
            "stack.toml: declares service sonarr, which belongs in services/sonarr.toml",
            "include entry x.toml: is not of the form services/<id>.toml",
            "services/qbittorrent.toml: holds service vpn, and its name says qbittorrent",
            "include entry services/gluetun.toml: names no file",
            "services/stray.toml: is in services/ and no include entry names it",
        ]
    );
}

#[test]
fn an_unknown_name_in_a_service_file_is_refused_as_unrecognised() {
    let mut services = both();
    services.insert(
        "gluetun.toml".to_owned(),
        service("gluetun").replace("bind = \"loopback\"", "bind = \"everywhere\""),
    );
    let named = match assemble(ROOT, &services) {
        Err(Failure::Unrecognised(named)) => said(&named),
        _ => Vec::new(),
    };
    assert_eq!(named.len(), 1, "got: {named:?}");
    assert!(
        named
            .iter()
            .all(|one| one.starts_with("service gluetun: bind:")),
        "got: {named:?}"
    );
}

#[test]
fn a_broken_rule_is_reported_before_an_unknown_name() {
    let mut services = both();
    services.insert(
        "gluetun.toml".to_owned(),
        service("gluetun").replace("bind = \"loopback\"", "bind = \"everywhere\""),
    );
    services.insert("stray.toml".to_owned(), String::new());
    assert_eq!(
        said(&faults(ROOT, &services)),
        ["services/stray.toml: is in services/ and no include entry names it"]
    );
}

/// A stack directory for one test, written fresh.
fn stack_dir(name: &str, written: &[(&str, &[u8])]) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("lemonfiber-assembly-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for (path, bytes) in written {
        let at = dir.join(path);
        let made = at
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&at, bytes));
        assert!(made.is_ok(), "{}: {made:?}", at.display());
    }
    dir
}

fn unreadable_path(found: &Result<String, Failure>) -> Option<&Path> {
    match found {
        Err(Failure::Unreadable { path, .. }) => Some(path),
        _ => None,
    }
}

#[test]
fn a_stack_directory_is_read_as_its_files_assembled() {
    let qbittorrent = service("qbittorrent");
    let gluetun = service("gluetun");
    let dir = stack_dir(
        "whole",
        &[
            ("stack.toml", ROOT.as_bytes()),
            ("services/qbittorrent.toml", qbittorrent.as_bytes()),
            ("services/gluetun.toml", gluetun.as_bytes()),
            ("services/README.md", b"not a service"),
            (
                "services/nested.toml/inside.toml",
                b"a directory, not a file",
            ),
        ],
    );
    assert_eq!(read(&dir).ok(), assemble(ROOT, &both()).ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stack_directory_with_no_services_directory_has_no_service_files() {
    let dir = stack_dir("flat", &[("stack.toml", ROOT.as_bytes())]);
    assert_eq!(
        said(&match read(&dir) {
            Err(Failure::Assembly(found)) => found,
            _ => Vec::new(),
        }),
        [
            "include entry services/qbittorrent.toml: names no file",
            "include entry services/gluetun.toml: names no file",
        ]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_root_names_the_file() {
    let dir = stack_dir("empty", &[]);
    assert_eq!(
        unreadable_path(&read(&dir)),
        Some(dir.join("stack.toml").as_path())
    );
}

#[test]
fn a_services_path_that_is_not_a_directory_names_it() {
    let dir = stack_dir(
        "services-a-file",
        &[("stack.toml", ROOT.as_bytes()), ("services", b"a file")],
    );
    assert_eq!(
        unreadable_path(&read(&dir)),
        Some(dir.join("services").as_path())
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_service_file_that_cannot_be_read_names_it() {
    let dir = stack_dir(
        "not-text",
        &[
            ("stack.toml", ROOT.as_bytes()),
            ("services/gluetun.toml", &[0xff, 0xfe, 0x00]),
        ],
    );
    assert_eq!(
        unreadable_path(&read(&dir)),
        Some(dir.join("services/gluetun.toml").as_path())
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unreadable_file_says_where_and_why() {
    let message = Failure::Unreadable {
        path: PathBuf::from("/stack/services/a.toml"),
        reason: "denied".to_owned(),
    }
    .to_string();
    assert_eq!(message, "/stack/services/a.toml could not be read: denied");
}

#[test]
fn a_broken_layout_lists_every_fault() {
    let message = Failure::Assembly(vec![
        Violation {
            location: "services/a.toml".to_owned(),
            message: "holds no [[service]]".to_owned(),
        },
        Violation {
            location: "include entry x".to_owned(),
            message: "names no file".to_owned(),
        },
    ])
    .to_string();
    assert_eq!(
        message,
        "the manifest's files break the contract:\n  services/a.toml: holds no [[service]]\n  include entry x: names no file"
    );
}

#[test]
fn a_fault_the_parser_places_nowhere_is_given_without_a_line() {
    let placed_nowhere = <toml::de::Error as serde::de::Error>::custom("the claim is not a table");
    assert_eq!(
        super::said("a\nb\n", &placed_nowhere),
        "the claim is not a table"
    );
}
