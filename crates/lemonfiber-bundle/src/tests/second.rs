use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::{hex, read, tree_digest, Refused, Stage, COPIES, FILE};

const REVISION: &str = "0123456789abcdef0123456789abcdef01234567";

/// The manifest of the tree-digest vector the catalogue holds its reader to.
const VECTOR: &str = "schema_version = 1

[plugin]
id = \"example\"

[[service]]
id = \"example\"

[[service]]
id = \"example-db\"

[[claim]]
capability = \"media.serve\"

[[claim.probe]]
id = \"catalogue\"
fixture = \"recordings/example/catalogue.json\"
";

/// An assembled part: one service, its starting file and its claim's recording.
const PART: &str = "schema_version = 1

[plugin]
id = \"sabnzbd\"
forms = [\"tv\"]

[[service]]
id = \"sabnzbd\"
provides = [\"download.usenet\"]
reaches = \"The Usenet providers you configure\"
asks_for = \"The articles each download is made of\"

[[ask]]
capability = \"indexer.search\"

[[claim]]
capability = \"download.usenet\"

[[claim.probe]]
id = \"queue\"
fixture = \"recordings/sabnzbd/queue.json\"
";

/// The part's files beside its manifest.
fn files() -> BTreeMap<String, Vec<u8>> {
    [
        ("config/sabnzbd/sabnzbd.ini", "[misc]\nport = 8080\n"),
        ("recordings/sabnzbd/queue.json", "{\"status\": 200}\n"),
    ]
    .into_iter()
    .map(|(path, text)| (path.to_owned(), text.as_bytes().to_vec()))
    .collect()
}

/// The stack tables the part sits among.
const TABLES: &str = "[[profile]]
id = \"usenet\"
name = \"Usenet\"
description = \"Usenet downloading\"
protocol = \"usenet\"

[[form]]
id = \"tv\"
name = \"TV\"
description = \"Television\"
profiles = [\"usenet\"]
";

/// The pin of `manifest` with `files`, its fields replaced by `fields`.
fn pin(manifest: &str, files: &BTreeMap<String, Vec<u8>>, fields: &[(&str, &str)]) -> String {
    let mut pin = vec![
        ("id", "\"sabnzbd\"".to_owned()),
        (
            "origin",
            "\"https://github.com/lemonfiber/plugin-sabnzbd\"".to_owned(),
        ),
        ("revision", format!("\"{REVISION}\"")),
        (
            "manifest",
            format!("\"sha256:{}\"", hex(manifest.as_bytes())),
        ),
        ("tree", format!("\"{}\"", tree_digest(manifest, files))),
        ("fills", "[\"download.usenet\"]".to_owned()),
        ("stage", "\"assembled\"".to_owned()),
        ("profile", "\"usenet\"".to_owned()),
        ("last_release", "\"2026-09-30\"".to_owned()),
    ];
    for (key, value) in fields {
        match pin.iter_mut().find(|(at, _)| at == key) {
            Some(slot) => slot.1 = (*value).to_owned(),
            None => pin.push((key, (*value).to_owned())),
        }
    }
    let mut body = String::new();
    for (key, value) in &pin {
        let _ = writeln!(body, "{key} = {value}");
    }
    format!("[[plugin]]\n{body}")
}

/// A catalogue for one test: `bundle`, and the copy `id` holding `manifest` and `files`.
fn catalogue(
    name: &str,
    bundle: &str,
    id: &str,
    manifest: &str,
    files: &BTreeMap<String, Vec<u8>>,
) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("lemonfiber-bundle-2-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let copy = root.join(COPIES).join(id);
    let mut written = std::fs::create_dir_all(&copy)
        .and_then(|()| std::fs::write(root.join(FILE), bundle))
        .and_then(|()| std::fs::write(copy.join("plugin.toml"), manifest));
    for (path, bytes) in files {
        let at = copy.join(path);
        written = written
            .and_then(|()| std::fs::create_dir_all(at.parent().unwrap_or(&copy)))
            .and_then(|()| std::fs::write(&at, bytes));
    }
    assert!(written.is_ok(), "{written:?}");
    root
}

/// The schema 2 bundle of the one part, with `manifest`, `files`, `fields` and `tables`.
fn bundle(
    manifest: &str,
    files: &BTreeMap<String, Vec<u8>>,
    fields: &[(&str, &str)],
    tables: &str,
) -> String {
    format!("schema = 2\n\n{tables}\n{}", pin(manifest, files, fields))
}

/// What the catalogue at `root` refuses, every rule named.
fn refused(root: &Path) -> Vec<String> {
    match read(root) {
        Err(Refused::Rules(said)) => said,
        other => vec![format!("not refused by rule: {other:?}")],
    }
}

/// The path the catalogue at `root` could not read.
fn unread(root: &Path) -> String {
    match read(root) {
        Err(Refused::Unread { path, .. }) => path,
        other => format!("read: {other:?}"),
    }
}

/// Whether the one-part catalogue with `manifest`, `files`, `fields` and `tables` reads.
fn reads(
    name: &str,
    manifest: &str,
    files: &BTreeMap<String, Vec<u8>>,
    fields: &[(&str, &str)],
    tables: &str,
) -> bool {
    read(&catalogue(
        name,
        &bundle(manifest, files, fields, tables),
        "sabnzbd",
        manifest,
        files,
    ))
    .is_ok()
}

/// Whether one refusal carries every word.
fn names(said: &[String], words: &[&str]) -> bool {
    said.iter()
        .any(|one| words.iter().all(|word| one.contains(word)))
}

/// What the one-part catalogue refuses with `manifest`, `files`, `fields` and `tables`.
fn broken(
    name: &str,
    manifest: &str,
    files: &BTreeMap<String, Vec<u8>>,
    fields: &[(&str, &str)],
    tables: &str,
) -> Vec<String> {
    refused(&catalogue(
        name,
        &bundle(manifest, files, fields, tables),
        "sabnzbd",
        manifest,
        files,
    ))
}

#[test]
fn the_tree_digest_is_the_catalogue_s_vector() {
    let files: BTreeMap<String, Vec<u8>> = [
        ("config/example/settings.ini", "[main]\nport = 8080\n"),
        (
            "config/example-db/init.sql",
            "CREATE TABLE seen (id INTEGER);\n",
        ),
        ("recordings/example/catalogue.json", "{\"status\": 200}\n"),
        ("conformance/media.serve.json", "{}\n"),
    ]
    .into_iter()
    .map(|(path, text)| (path.to_owned(), text.as_bytes().to_vec()))
    .collect();
    assert_eq!(
        tree_digest(VECTOR, &files),
        "sha256:083641a4cacc0289f6d0acc96e7e1b67b2dafc17b8d5df47265eaaaf5ce35a77"
    );
}

#[test]
fn an_assembled_part_is_read_with_its_tree_and_the_stack_s_tables() {
    let root = catalogue(
        "assembled",
        &bundle(PART, &files(), &[], TABLES),
        "sabnzbd",
        PART,
        &files(),
    );
    let read = read(&root);
    assert!(read.is_ok(), "{read:?}");
    let Ok((bundle, held)) = read else {
        return;
    };
    assert_eq!(bundle.profile.len(), 1);
    assert_eq!(bundle.form.len(), 1);
    let one = held.first();
    assert_eq!(one.map(|one| one.copy.as_str()), Some(PART));
    assert_eq!(one.map(|one| one.tree.clone()), Some(files()));
    assert_eq!(one.and_then(|one| one.pin.stage), Some(Stage::Assembled));
    assert_eq!(one.and_then(|one| one.pin.release.clone()), None);
}

#[test]
fn a_copy_that_is_not_the_pinned_tree_is_refused() {
    let mut changed = files();
    changed.insert(
        "config/sabnzbd/sabnzbd.ini".to_owned(),
        b"[misc]\nport = 9090\n".to_vec(),
    );
    let root = catalogue(
        "retreed",
        &bundle(PART, &files(), &[], TABLES),
        "sabnzbd",
        PART,
        &changed,
    );
    assert!(names(&refused(&root), &["sabnzbd/", "plugin sabnzbd.tree"]));
}

#[test]
fn a_file_outside_the_tree_is_refused() {
    let mut stray = files();
    stray.insert("README.md".to_owned(), b"# SABnzbd\n".to_vec());
    let said = broken("stray", PART, &stray, &[], TABLES);
    assert!(
        names(
            &said,
            &["sabnzbd/README.md", "is not part of plugin sabnzbd's tree"]
        ),
        "{said:?}"
    );
}

#[test]
fn starting_files_past_the_bound_are_refused() {
    let mut many = files();
    for at in 0..65 {
        many.insert(format!("config/sabnzbd/{at}.ini"), b"x\n".to_vec());
    }
    let said = broken("many", PART, &many, &[], TABLES);
    assert!(
        names(&said, &["service sabnzbd ships", "past 64 files"]),
        "{said:?}"
    );
}

#[test]
fn an_assembled_part_of_two_services_or_an_adapter_is_refused() {
    let two = PART.replace("[[ask]]", "[[service]]\nid = \"sabnzbd-sidecar\"\nreaches = \"Nowhere\"\nasks_for = \"Nothing\"\n\n[[ask]]");
    let said = broken("two", &two, &files(), &[], TABLES);
    assert!(
        names(
            &said,
            &["plugin sabnzbd is assembled", "exactly one service"]
        ),
        "{said:?}"
    );
    let adapter = PART.replace(
        "provides = [",
        "speaks = [\"download.usenet@1\"]\nprovides = [",
    );
    let said = broken("adapter", &adapter, &files(), &[], TABLES);
    assert!(
        names(&said, &["plugin sabnzbd is assembled", "speaking no"]),
        "{said:?}"
    );
}

#[test]
fn an_installed_part_replaces_the_service_of_its_id() {
    let said = broken(
        "installed",
        PART,
        &files(),
        &[("stage", "\"installed\"")],
        TABLES,
    );
    assert!(
        names(
            &said,
            &["plugin sabnzbd is installed", "replaces = \"sabnzbd\""]
        ),
        "{said:?}"
    );
    let replacing = PART.replace("provides = [", "replaces = \"sabnzbd\"\nprovides = [");
    let said = broken(
        "replacing",
        &replacing,
        &files(),
        &[("stage", "\"installed\"")],
        TABLES,
    );
    assert!(!names(&said, &["plugin sabnzbd is installed"]), "{said:?}");
}

#[test]
fn a_part_s_forms_are_the_ones_holding_its_profile() {
    let said = broken(
        "forms",
        &PART.replace("forms = [\"tv\"]", "forms = [\"films\"]"),
        &files(),
        &[],
        TABLES,
    );
    assert!(
        names(&said, &["[plugin].forms", "profile usenet"]),
        "{said:?}"
    );
    let said = broken(
        "profile",
        PART,
        &files(),
        &[("profile", "\"torrent\"")],
        TABLES,
    );
    assert!(
        names(
            &said,
            &["plugin sabnzbd.profile torrent is not a declared profile"]
        ),
        "{said:?}"
    );
}

#[test]
fn a_first_party_service_says_where_it_reaches_and_what_it_asks_for() {
    let silent = PART.replace("asks_for = \"The articles each download is made of\"\n", "");
    let said = broken("silent", &silent, &files(), &[], TABLES);
    assert!(
        names(&said, &["service sabnzbd says nothing of where it reaches"]),
        "{said:?}"
    );
}

#[test]
fn a_first_party_service_reaching_nothing_says_so_with_an_empty_reaches() {
    let local = PART.replace(
        "reaches = \"The Usenet providers you configure\"",
        "reaches = \"\"",
    );
    let said = broken("local", &local, &files(), &[], TABLES);
    assert!(
        !names(&said, &["says nothing of where it reaches"]),
        "{said:?}"
    );
    let blank = PART.replace(
        "asks_for = \"The articles each download is made of\"",
        "asks_for = \" \"",
    );
    let said = broken("blank", &blank, &files(), &[], TABLES);
    assert!(
        names(&said, &["service sabnzbd says nothing of where it reaches"]),
        "{said:?}"
    );
    let unsaid = PART.replace("reaches = \"The Usenet providers you configure\"\n", "");
    let said = broken("unsaid", &unsaid, &files(), &[], TABLES);
    assert!(
        names(&said, &["service sabnzbd says nothing of where it reaches"]),
        "{said:?}"
    );
}

#[test]
fn schema_2_requires_the_tree_stage_profile_and_last_release() {
    let fields = ["tree", "stage", "profile", "last_release"];
    let pinned = bundle(PART, &files(), &[], TABLES)
        .lines()
        .filter(|line| {
            !fields
                .iter()
                .any(|field| line.starts_with(&format!("{field} =")))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let said = refused(&catalogue("unstaged", &pinned, "sabnzbd", PART, &files()));
    for field in fields {
        assert!(
            names(&said, &[&format!("plugin sabnzbd.{field} is missing")]),
            "{said:?}"
        );
    }
    let said = broken(
        "day",
        PART,
        &files(),
        &[("last_release", "\"30-09-2026\"")],
        TABLES,
    );
    assert!(names(&said, &["last_release", "is not a day"]), "{said:?}");
}

#[test]
fn a_choice_of_filler_names_an_ask_its_asker_makes_and_a_filler_providing_it() {
    let choice = |by: &str, asks: &str, filler: &str| {
        format!("{TABLES}\n[[wiring]]\nby = \"{by}\"\nasks = \"{asks}\"\nfilled_by = \"{filler}\"\nwhy = \"The stack's choice\"\n")
    };
    let said = broken(
        "unasked",
        PART,
        &files(),
        &[],
        &choice("sabnzbd", "media.serve", "sabnzbd"),
    );
    assert!(
        names(
            &said,
            &["wiring by sabnzbd chooses for media.serve", "does not ask"]
        ),
        "{said:?}"
    );
    let said = broken(
        "unfilled",
        PART,
        &files(),
        &[],
        &choice("sabnzbd", "indexer.search", "sabnzbd"),
    );
    assert!(
        names(
            &said,
            &["chooses sabnzbd for indexer.search", "provides or speaks"]
        ),
        "{said:?}"
    );
    let said = broken(
        "filled",
        PART,
        &files(),
        &[],
        &choice("sabnzbd", "download.usenet", "sabnzbd"),
    );
    assert!(!names(&said, &["provides or speaks"]), "{said:?}");
}

#[test]
fn a_wiring_is_a_choice_or_a_link_by_name_between_pinned_services() {
    let wiring = |body: &str| format!("{TABLES}\n[[wiring]]\n{body}");
    let said = broken(
        "neither",
        PART,
        &files(),
        &[],
        &wiring("by = \"sabnzbd\"\nasks = \"indexer.search\"\nwhy = \"Because\"\n"),
    );
    assert!(names(&said, &["is neither a choice of filler"]), "{said:?}");
    let said = broken(
        "ghost",
        PART,
        &files(),
        &[],
        &wiring("by = \"ghost\"\nto = \"sabnzbd\"\nwhy = \"Because\"\n"),
    );
    assert!(
        names(&said, &["names ghost, which no pinned plugin declares"]),
        "{said:?}"
    );
    let said = broken(
        "each",
        PART,
        &files(),
        &[],
        &wiring("by = \"sabnzbd\"\nto = \"sabnzbd\"\neach = true\nwhy = \" \"\n"),
    );
    assert!(names(&said, &["declares each"]), "{said:?}");
    assert!(names(&said, &["gives no why"]), "{said:?}");
}

#[test]
fn the_stack_s_tables_name_what_is_declared() {
    let tables = format!(
        "{TABLES}\n[[form]]\nid = \"tv\"\nname = \"Again\"\ndescription = \"Twice\"\nprofiles = [\"subs\"]\n\n[[removed]]\nid = \"sabnzbd\"\nremoved_in = \"0.1.0\"\nreason = \"Gone\"\nreplaced_by = \"nzbget\"\n"
    );
    let said = broken("tables", PART, &files(), &[], &tables);
    for words in [
        &["form \"tv\" is not an id or is declared twice"][..],
        &["form tv names profile subs"],
        &["removed sabnzbd is a pinned plugin"],
        &["replaced by nzbget, which is not a pinned plugin"],
    ] {
        assert!(names(&said, words), "{words:?} in {said:?}");
    }
}

#[test]
fn schema_1_declares_no_stack_table_and_no_schema_2_field() {
    let first = format!("schema = 1\n\n{TABLES}\n{}", pin(PART, &files(), &[]));
    let said = refused(&catalogue("first", &first, "sabnzbd", PART, &files()));
    for words in [
        &["[[profile]] is declared at schema 1"][..],
        &["plugin sabnzbd.release is missing"],
        &["plugin sabnzbd.tree is declared at schema 1"],
    ] {
        assert!(names(&said, words), "{words:?} in {said:?}");
    }
}

#[test]
fn a_manifest_that_is_not_toml_declares_nothing() {
    let said = broken("untoml", "schema_version = [\n", &files(), &[], TABLES);
    for words in [
        &["sabnzbd/plugin.toml is not the manifest of plugin sabnzbd"][..],
        &["sabnzbd/config/sabnzbd/sabnzbd.ini is not part of plugin sabnzbd's tree"],
    ] {
        assert!(names(&said, words), "{words:?} in {said:?}");
    }
}

#[test]
fn a_conformance_recording_is_part_of_the_tree_and_its_namesake_is_not() {
    let mut recorded = files();
    recorded.insert(
        "conformance/download.usenet.json".to_owned(),
        b"{}\n".to_vec(),
    );
    assert!(reads("recorded", PART, &recorded, &[], TABLES));
    recorded.insert("conformance.json".to_owned(), b"{}\n".to_vec());
    let said = broken("namesake", PART, &recorded, &[], TABLES);
    assert_eq!(
        said,
        ["sabnzbd/conformance.json is not part of plugin sabnzbd's tree"]
    );
}

#[test]
fn starting_files_up_to_the_bound_are_read() {
    let size = 1024 * 1024 / 64;
    let mut bounded = files();
    bounded.remove("config/sabnzbd/sabnzbd.ini");
    for at in 0..64 {
        bounded.insert(format!("config/sabnzbd/{at}.ini"), vec![b'x'; size]);
    }
    assert!(reads("bounded", PART, &bounded, &[], TABLES));
    let mut heavy = bounded.clone();
    heavy.insert("config/sabnzbd/0.ini".to_owned(), vec![b'x'; size + 1]);
    let said = broken("heavy", PART, &heavy, &[], TABLES);
    assert!(
        names(&said, &["ships 64 starting files of 1048577 bytes"]),
        "{said:?}"
    );
    let mut many = bounded;
    many.insert("config/sabnzbd/64.ini".to_owned(), Vec::new());
    let said = broken("sixty-five", PART, &many, &[], TABLES);
    assert!(
        names(&said, &["ships 65 starting files of 1048576 bytes"]),
        "{said:?}"
    );
}

#[test]
fn a_profile_is_an_id_declared_once() {
    let tables = format!(
        "{TABLES}\n[[profile]]\nid = \"usenet\"\nname = \"Again\"\ndescription = \"Twice\"\nprotocol = \"usenet\"\n\n[[profile]]\nid = \"Usenet Two\"\nname = \"Spaced\"\ndescription = \"Not an id\"\nprotocol = \"usenet\"\n"
    );
    let said = broken("profiles", PART, &files(), &[], &tables);
    for words in [
        &["profile \"usenet\" is not an id or is declared twice"][..],
        &["profile \"Usenet Two\" is not an id or is declared twice"],
    ] {
        assert!(names(&said, words), "{words:?} in {said:?}");
    }
}

#[test]
fn a_removed_service_replaced_by_a_pinned_part_or_by_none_is_read() {
    let tables = format!(
        "{TABLES}\n[[removed]]\nid = \"nzbget\"\nremoved_in = \"0.1.0\"\nreason = \"Gone\"\nreplaced_by = \"sabnzbd\"\n\n[[removed]]\nid = \"sickrage\"\nremoved_in = \"0.1.0\"\nreason = \"Unmaintained\"\n"
    );
    assert!(reads("replaced", PART, &files(), &[], &tables));
}

#[test]
fn a_link_by_name_names_a_pinned_service() {
    let link = |to: &str| {
        format!("{TABLES}\n[[wiring]]\nby = \"sabnzbd\"\nto = \"{to}\"\nwhy = \"Because\"\n")
    };
    assert!(reads("linked-in", PART, &files(), &[], &link("sabnzbd")));
    let said = broken("linked-out", PART, &files(), &[], &link("ghost"));
    assert_eq!(
        said,
        ["wiring by sabnzbd names ghost, which no pinned plugin declares"]
    );
}

#[test]
fn a_choice_reads_an_ask_its_named_service_makes_and_a_filler_speaking_it() {
    let speaking = PART
        .replace(
            "provides = [",
            "replaces = \"sabnzbd\"\nspeaks = [\"indexer.search@1\"]\nprovides = [",
        )
        .replace(
            "capability = \"indexer.search\"",
            "capability = \"indexer.search\"\nservice = \"sabnzbd\"",
        );
    let choice = format!("{TABLES}\n[[wiring]]\nby = \"sabnzbd\"\nasks = \"indexer.search\"\nfilled_by = \"sabnzbd\"\nwhy = \"The stack's choice\"\n");
    assert!(reads(
        "speaking",
        &speaking,
        &files(),
        &[("stage", "\"installed\"")],
        &choice
    ));
}

#[test]
fn a_choice_needs_an_ask_made_once_by_a_known_asker() {
    let choice = format!("{TABLES}\n[[wiring]]\nby = \"sabnzbd\"\nasks = \"indexer.search\"\nfilled_by = \"sabnzbd\"\nwhy = \"The stack's choice\"\n");
    let unnamed = PART.replace(
        "[[ask]]",
        "[[service]]\nid = \"sabnzbd-sidecar\"\nreaches = \"Nowhere\"\nasks_for = \"Nothing\"\n\n[[ask]]",
    );
    let each = PART.replace(
        "capability = \"indexer.search\"",
        "capability = \"indexer.search\"\neach = true",
    );
    for (name, manifest) in [("unnamed", unnamed), ("each", each)] {
        let said = broken(name, &manifest, &files(), &[], &choice);
        assert!(
            names(
                &said,
                &[
                    "wiring by sabnzbd chooses for indexer.search",
                    "does not ask once"
                ]
            ),
            "{name}: {said:?}"
        );
    }
}

mod assembled;
mod reading;

/// The fence's image as the bundle pins it, before the pins.
fn fence(image: &str, tag: &str, digest: &str) -> String {
    format!("[fence]\nimage = \"{image}\"\ntag = \"{tag}\"\ndigest = \"{digest}\"\n\n")
}

/// A part with a fenced service is read only where the bundle pins the fence's image, and
/// that pin is held to where the fence is published, a release or a commit image, and a
/// digest.
#[test]
fn a_fenced_part_needs_the_fence_image_pinned_and_held_to_its_shape() {
    let fenced = PART.replace(
        "asks_for = \"The articles each download is made of\"",
        "asks_for = \"The articles each download is made of\"\nnetwork = { fenced = true }",
    );
    let files = files();
    let digest = format!("sha256:{}", "a".repeat(64));
    let with = |pinned: &str| format!("{TABLES}\n{pinned}");
    let catalogued = |name: &str, tables: &str| {
        catalogue(
            name,
            &bundle(&fenced, &files, &[], tables),
            "sabnzbd",
            &fenced,
            &files,
        )
    };

    let unpinned = refused(&catalogued("fence-unpinned", TABLES));
    assert!(
        unpinned
            .iter()
            .any(|line| line.contains("service sabnzbd is fenced") && line.contains("[fence]")),
        "{unpinned:?}"
    );
    let pinned = read(&catalogued(
        "fence-pinned",
        &with(&fence("ghcr.io/lemonfiber/fence", "0.18.0", &digest)),
    ));
    assert_eq!(
        pinned
            .ok()
            .and_then(|(bundle, _)| bundle.fence)
            .map(|image| image.reference()),
        Some(format!("ghcr.io/lemonfiber/fence:0.18.0@{digest}"))
    );
    let pre = read(&catalogued(
        "fence-pre",
        &with(&fence("ghcr.io/lemonfiber/fence", "0.18.0-beta.1", &digest)),
    ));
    assert!(pre.is_ok(), "{pre:?}");
    let commit = read(&catalogued(
        "fence-commit",
        &with(&fence(
            "ghcr.io/lemonfiber/fence",
            "main-8de8b3aa2b24",
            &digest,
        )),
    ));
    assert!(commit.is_ok(), "{commit:?}");

    for (name, pinned, said) in [
        (
            "fence-elsewhere",
            fence("ghcr.io/else/fence", "0.18.0", &digest),
            "fence.image is ghcr.io/else/fence",
        ),
        (
            "fence-tag",
            fence("ghcr.io/lemonfiber/fence", "v0.18.0", &digest),
            "fence.tag v0.18.0 is not a release",
        ),
        (
            "fence-short-commit",
            fence("ghcr.io/lemonfiber/fence", "main-8de8b3aa2b2", &digest),
            "fence.tag main-8de8b3aa2b2 is not a release",
        ),
        (
            "fence-upper-commit",
            fence("ghcr.io/lemonfiber/fence", "main-8DE8B3AA2B24", &digest),
            "fence.tag main-8DE8B3AA2B24 is not a release",
        ),
        (
            "fence-branch",
            fence("ghcr.io/lemonfiber/fence", "next-8de8b3aa2b24", &digest),
            "fence.tag next-8de8b3aa2b24 is not a release",
        ),
        (
            "fence-digest",
            fence("ghcr.io/lemonfiber/fence", "0.18.0", "sha256:abc"),
            "fence.digest is not written",
        ),
    ] {
        let broken = refused(&catalogued(name, &with(&pinned)));
        assert!(
            broken.iter().any(|line| line.contains(said)),
            "{name}: {broken:?}"
        );
    }
}
