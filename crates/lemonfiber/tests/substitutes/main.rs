//! The substitutes each named capability is proved by, as `substitutes.toml` pins them.
//!
//! Run as it is, it holds the file to its shape: every arm names one of those capabilities,
//! a substitute other than the plugin the bundle fills that capability with, and a
//! pin a checkout can be made at. Run by the substitutes job with `SUBSTITUTE_ARM` naming
//! one arm and `SUBSTITUTE_TREE` a checkout of its origin at its revision, it holds that
//! checkout to what the arm says the substitute is.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The arm the job runs, where it runs one.
const ARM: &str = "SUBSTITUTE_ARM";

/// Where the job checked the arm's substitute out.
const TREE: &str = "SUBSTITUTE_TREE";

/// The capabilities a substitute proves.
const NAMED: [&str; 4] = [
    "media.serve",
    "indexer.search",
    "library.curate",
    "identity.source",
];

/// The capability whose substitute may run unclaimed.
const CLAIMABLE: &str = "media.serve";

/// Where every first-party plugin's repository lives, before its id.
const ORIGINS: &str = "https://github.com/lemonfiber/plugin-";

/// One arm, as the file pins it.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Substitute {
    arm: String,
    capability: String,
    plugin: String,
    origin: String,
    revision: String,
    stands_in_for: String,
    media_type: String,
    claimed: bool,
}

/// The file.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Pinned {
    schema: u32,
    substitute: Vec<Substitute>,
}

/// The workspace's root, where the file is.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The TOML document at `path`, read as `T`, or why it does not read.
fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let written =
        std::fs::read_to_string(path).map_err(|why| format!("{}: {why}", path.display()))?;
    toml::from_str(&written).map_err(|why| format!("{}: {why}", path.display()))
}

/// The pins, and the document at `beside`, both read.
fn read_with(beside: &Path) -> Result<(Pinned, toml::Value), String> {
    Ok((read(&root().join("substitutes.toml"))?, read(beside)?))
}

/// The strings the array at `key` of `table` holds.
fn strings<'a>(table: &'a toml::Value, key: &str) -> Vec<&'a str> {
    table
        .get(key)
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .collect()
}

/// The capabilities the bundle's plugin `id` fills by default.
fn fills<'a>(bundle: &'a toml::Value, id: &str) -> Vec<&'a str> {
    bundle
        .get("plugin")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter(|plugin| plugin.get("id").and_then(toml::Value::as_str) == Some(id))
        .flat_map(|plugin| strings(plugin, "fills"))
        .collect()
}

/// Whether `revision` is a full commit, written as git writes it.
fn full_commit(revision: &str) -> bool {
    revision.len() == 40
        && revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[test]
fn every_arm_stands_a_substitute_in_for_what_the_bundle_fills_the_capability_with() {
    let read = read_with(&root().join("assets/plugins/bundle/bundle.toml"));
    assert!(read.is_ok(), "{read:?}");
    let Ok((pinned, bundle)) = read else {
        return;
    };
    assert_eq!(pinned.schema, 1);
    let mut arms = BTreeSet::new();
    for one in &pinned.substitute {
        assert!(arms.insert(one.arm.as_str()), "{} is named twice", one.arm);
        assert!(NAMED.contains(&one.capability.as_str()), "{one:?}");
        assert_ne!(one.plugin, one.stands_in_for, "{one:?}");
        assert!(
            fills(&bundle, &one.stands_in_for).contains(&one.capability.as_str()),
            "{one:?}"
        );
        assert_eq!(one.origin, format!("{ORIGINS}{}", one.plugin), "{one:?}");
        assert!(full_commit(&one.revision), "{one:?}");
        assert!(
            one.arm
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'-'),
            "{one:?}"
        );
        assert!(one.claimed || one.capability == CLAIMABLE, "{one:?}");
    }
}

#[test]
fn a_revision_is_a_full_commit_and_nothing_else() {
    assert!(full_commit(&"0a".repeat(20)));
    for revision in [
        "0a".repeat(19),
        "0A".repeat(20),
        "g".repeat(40),
        "main".to_owned(),
    ] {
        assert!(!full_commit(&revision), "{revision}");
    }
}

#[test]
fn the_arm_the_job_runs_is_the_substitute_its_checkout_holds() {
    let (Ok(arm), Ok(tree)) = (std::env::var(ARM), std::env::var(TREE)) else {
        return;
    };
    let read = read_with(&Path::new(&tree).join("plugin.toml"));
    assert!(read.is_ok(), "{arm}: {read:?}");
    let Ok((pinned, manifest)) = read else {
        return;
    };
    let one = pinned.substitute.iter().find(|one| one.arm == arm);
    assert!(one.is_some(), "substitutes.toml has no arm {arm}");
    let Some(one) = one else {
        return;
    };
    assert_eq!(
        manifest
            .get("plugin")
            .and_then(|plugin| plugin.get("id"))
            .and_then(toml::Value::as_str),
        Some(one.plugin.as_str()),
        "{arm}"
    );
    let providing = manifest
        .get("service")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .find(|service| strings(service, "provides").contains(&one.capability.as_str()));
    assert!(
        providing.is_some(),
        "{arm}: no service of {} provides {}",
        one.plugin,
        one.capability
    );
    let files = providing.map(|service| strings(service, "media_types"));
    assert!(
        files
            .as_ref()
            .is_some_and(|files| files.is_empty() || files.contains(&one.media_type.as_str())),
        "{arm}: {} files {files:?}, not {}",
        one.plugin,
        one.media_type
    );
}

#[test]
fn the_workflow_runs_one_job_for_every_arm_and_no_other() {
    let pinned = read::<Pinned>(&root().join("substitutes.toml"));
    let written = std::fs::read_to_string(root().join(".github/workflows/substitutes.yml"));
    assert!(pinned.is_ok() && written.is_ok(), "{pinned:?} {written:?}");
    let (Ok(pinned), Ok(written)) = (pinned, written) else {
        return;
    };
    let workflow: Result<serde_norway::Value, _> = serde_norway::from_str(&written);
    assert!(workflow.is_ok(), "{workflow:?}");
    let Ok(workflow) = workflow else {
        return;
    };
    let mut run: Vec<&str> = workflow
        .get("jobs")
        .and_then(|jobs| jobs.get("arm"))
        .and_then(|arm| arm.get("strategy"))
        .and_then(|strategy| strategy.get("matrix"))
        .and_then(|matrix| matrix.get("arm"))
        .and_then(serde_norway::Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(serde_norway::Value::as_str)
        .collect();
    let mut arms: Vec<&str> = pinned
        .substitute
        .iter()
        .map(|one| one.arm.as_str())
        .collect();
    run.sort_unstable();
    arms.sort_unstable();
    assert_eq!(run, arms);
}
