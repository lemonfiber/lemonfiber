//! The substitutes each named capability is proved by, as `substitutes.toml` pins them.
//!
//! Run as it is, it holds the file to its shape: every arm names one of those capabilities,
//! a substitute other than the plugin the bundle fills that capability with, and a
//! pin a checkout can be made at. Run by the substitutes job with `SUBSTITUTE_ARM` naming
//! one arm and `SUBSTITUTE_TREE` a checkout of its origin at its revision, it holds that
//! checkout to what the arm says the substitute is.

mod live;
mod withheld;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The arm the job runs, where it runs one.
const ARM: &str = "SUBSTITUTE_ARM";

/// Where the job checked the arm's substitute out.
const TREE: &str = "SUBSTITUTE_TREE";

/// The capabilities a substitute proves.
const NAMED: [&str; 4] = [SERVES, "indexer.search", CURATES, live::IDENTIFIES];

/// The further capabilities an arm proves, each as those [`NAMED`] are proved.
const FURTHER: [&str; 1] = [SOLVES];

/// The capability a challenge solver fills.
const SOLVES: &str = lemonfiber_contract::capabilities::indexer::proxy::CAPABILITY;

/// The download capabilities an arm proves, each as those [`NAMED`] are proved, with the
/// downloads its arm runs with.
const DOWNLOADS: [(&str, &live::Downloads); 2] = [
    ("download.usenet", &live::USENET),
    ("download.torrent", &live::TORRENT),
];

/// The workflow that runs every arm, from the workspace's root.
const WORKFLOW: &str = ".github/workflows/substitutes.yml";

/// The capability a media server fills.
const SERVES: &str = lemonfiber_manifest::container::MEDIA_SERVE;

/// The capability whose substitute may run unclaimed.
const CLAIMABLE: &str = SERVES;

/// The forms a media server standing in is started with where no household request is run
/// for its media type: the library alone.
const SERVING_ALONE: [&str; 1] = ["library"];

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
    media_type: Option<String>,
    claimed: bool,
}

/// One curator the bundle pins, run live through its own request flow.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Curator {
    arm: String,
    plugin: String,
    origin: String,
    revision: String,
    media_type: String,
}

/// The file.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Pinned {
    schema: u32,
    substitute: Vec<Substitute>,
    #[serde(default)]
    curator: Vec<Curator>,
}

impl Pinned {
    /// Every arm's name, a substitute's and a curator's.
    fn arms(&self) -> Vec<&str> {
        self.substitute
            .iter()
            .map(|one| one.arm.as_str())
            .chain(self.curator.iter().map(|one| one.arm.as_str()))
            .collect()
    }
}

/// The capability a curator arm proves.
const CURATES: &str = "library.curate";

impl Substitute {
    /// It put in place, given `settings`.
    fn substitution(&self, settings: Vec<(String, String)>) -> live::Substitution {
        live::Substitution {
            plugin: self.plugin.clone(),
            capability: self.capability.clone(),
            also: Vec::new(),
            stands_in_for: self.stands_in_for.clone(),
            settings,
        }
    }
}

/// The arm the job runs, the substitute it pins where that proves one of `capabilities`,
/// and the household request run for its media type, or nothing where the job runs no
/// such arm.
///
/// # Panics
///
/// Where the file does not read, or no household request is run for the arm's media type.
fn standing_in(capabilities: &[&str]) -> Option<(String, Substitute, &'static live::Asked)> {
    let arm = std::env::var(ARM).ok()?;
    let pinned = read::<Pinned>(&root().join("substitutes.toml"));
    assert!(pinned.is_ok(), "{pinned:?}");
    let one = pinned
        .ok()?
        .substitute
        .into_iter()
        .find(|one| one.arm == arm && capabilities.contains(&one.capability.as_str()))?;
    let asked = live::ASKED
        .iter()
        .find(|asked| Some(asked.media) == one.media_type.as_deref());
    assert!(asked.is_some(), "{one:?}");
    Some((arm, one, asked?))
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

/// The revision the bundle pins its plugin `id` at.
fn bundled_at<'a>(bundle: &'a toml::Value, id: &str) -> Option<&'a str> {
    pinned_in(bundle, "plugin", id)
}

/// The revision the bundle's `table` pins the plugin `id` at.
fn pinned_in<'a>(bundle: &'a toml::Value, table: &str, id: &str) -> Option<&'a str> {
    bundle
        .get(table)
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .find(|plugin| plugin.get("id").and_then(toml::Value::as_str) == Some(id))
        .and_then(|plugin| plugin.get("revision"))
        .and_then(toml::Value::as_str)
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
        assert!(
            NAMED.contains(&one.capability.as_str())
                || FURTHER.contains(&one.capability.as_str())
                || DOWNLOADS
                    .iter()
                    .any(|(capability, _)| *capability == one.capability),
            "{one:?}"
        );
        assert_eq!(
            one.media_type.is_none(),
            one.capability == SOLVES,
            "{one:?}: a challenge solver alone files no media"
        );
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
        if let Some(offered) = pinned_in(&bundle, "offered", &one.plugin) {
            assert_eq!(
                offered, one.revision,
                "{one:?}: the bundle offers it at {offered}"
            );
        }
    }
}

#[test]
fn every_curator_arm_runs_a_curator_the_bundle_pins_at_its_pin() {
    let read = read_with(&root().join("assets/plugins/bundle/bundle.toml"));
    assert!(read.is_ok(), "{read:?}");
    let Ok((pinned, bundle)) = read else {
        return;
    };
    let arms = pinned.arms();
    let named: BTreeSet<&str> = arms.iter().copied().collect();
    assert_eq!(named.len(), arms.len(), "an arm is named twice: {arms:?}");
    for one in &pinned.curator {
        assert!(fills(&bundle, &one.plugin).contains(&CURATES), "{one:?}");
        assert_eq!(
            bundled_at(&bundle, &one.plugin),
            Some(one.revision.as_str()),
            "{one:?}"
        );
        assert_eq!(one.origin, format!("{ORIGINS}{}", one.plugin), "{one:?}");
        assert!(
            live::ASKED
                .iter()
                .any(|asked| asked.media == one.media_type),
            "{one:?}: no household request is run for {}",
            one.media_type
        );
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
    let one = pinned
        .substitute
        .iter()
        .find(|one| one.arm == arm)
        .map(|one| {
            (
                one.plugin.as_str(),
                one.capability.as_str(),
                one.media_type.as_deref(),
            )
        })
        .or_else(|| {
            pinned
                .curator
                .iter()
                .find(|one| one.arm == arm)
                .map(|one| (one.plugin.as_str(), CURATES, Some(one.media_type.as_str())))
        });
    assert!(one.is_some(), "substitutes.toml has no arm {arm}");
    let Some((plugin, capability, media_type)) = one else {
        return;
    };
    assert_eq!(
        manifest
            .get("plugin")
            .and_then(|plugin| plugin.get("id"))
            .and_then(toml::Value::as_str),
        Some(plugin),
        "{arm}"
    );
    let providing = manifest
        .get("service")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .find(|service| strings(service, "provides").contains(&capability));
    assert!(
        providing.is_some(),
        "{arm}: no service of {plugin} provides {capability}"
    );
    let files = providing.map(|service| strings(service, "media_types"));
    assert!(
        files.as_ref().is_some_and(|files| match media_type {
            Some(media_type) => files.is_empty() || files.contains(&media_type),
            None => files.is_empty(),
        }),
        "{arm}: {plugin} files {files:?}, not {media_type:?}"
    );
}

#[test]
fn the_workflow_runs_one_job_for_every_arm_and_no_other() {
    let pinned = read::<Pinned>(&root().join("substitutes.toml"));
    let written = std::fs::read_to_string(root().join(WORKFLOW));
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
    let mut arms = pinned.arms();
    run.sort_unstable();
    arms.sort_unstable();
    assert_eq!(run, arms);
}

#[test]
fn a_curator_arm_carries_a_household_request_to_a_grab_and_undoes_it() {
    let Ok(arm) = std::env::var(ARM) else {
        return;
    };
    let pinned = read::<Pinned>(&root().join("substitutes.toml"));
    assert!(pinned.is_ok(), "{pinned:?}");
    let Ok(pinned) = pinned else {
        return;
    };
    let Some(curator) = pinned.curator.iter().find(|one| one.arm == arm) else {
        return;
    };
    let asked = live::ASKED
        .iter()
        .find(|asked| asked.media == curator.media_type);
    assert!(asked.is_some(), "{curator:?}");
    let Some(asked) = asked else {
        return;
    };
    live::run(&scratch(&arm), asked, &live::USENET, None);
}

/// The job's secret holding the operator's `TheTVDB` key, which Medusa looks a series up
/// with.
const THETVDB_KEY: &str = "SUBSTITUTES_THETVDB_KEY";

/// The job's secret holding the subscriber PIN a user-supported `TheTVDB` key needs.
const THETVDB_PIN: &str = "SUBSTITUTES_THETVDB_PIN";

/// The settings a curator standing in is given, from the job's secrets, or why it cannot
/// run where one it needs is not set.
fn settings_of(plugin: &str) -> Result<Vec<(String, String)>, String> {
    if plugin != "medusa" {
        return Ok(Vec::new());
    }
    let key = std::env::var(THETVDB_KEY)
        .ok()
        .filter(|key| !key.is_empty())
        .ok_or_else(|| format!("{THETVDB_KEY} is not set, so Medusa cannot look up a series"))?;
    let pin = std::env::var(THETVDB_PIN)
        .ok()
        .filter(|pin| !pin.is_empty());
    let mut settings = vec![
        (
            "medusa:thetvdb-lookup".to_owned(),
            if pin.is_some() { "on-with-pin" } else { "on" }.to_owned(),
        ),
        ("medusa:thetvdb-key".to_owned(), key),
    ];
    settings.extend(pin.map(|pin| ("medusa:thetvdb-pin".to_owned(), pin)));
    Ok(settings)
}

#[test]
fn a_curator_standing_in_carries_a_household_request_to_a_grab_and_undoes_it() {
    let Some((arm, one, asked)) = standing_in(&[CURATES]) else {
        return;
    };
    let settings = match settings_of(&one.plugin) {
        Ok(settings) => settings,
        Err(why) => {
            println!("{arm} does not run: {why}");
            return;
        }
    };
    let substitution = one.substitution(settings);
    live::run(&scratch(&arm), asked, &live::USENET, Some(&substitution));
}

#[test]
fn a_media_server_standing_in_holds_a_title_written_into_the_library_it_was_furnished_and_undoes_it(
) {
    let Ok(arm) = std::env::var(ARM) else {
        return;
    };
    let pinned = read::<Pinned>(&root().join("substitutes.toml"));
    assert!(pinned.is_ok(), "{pinned:?}");
    let Some(one) = pinned.ok().and_then(|pinned| {
        pinned
            .substitute
            .into_iter()
            .find(|one| one.arm == arm && one.capability == SERVES && one.claimed)
    }) else {
        return;
    };
    assert!(one.media_type.is_some(), "{one:?}");
    let Some(media) = one.media_type.as_deref() else {
        return;
    };
    let forms = live::ASKED
        .iter()
        .find(|asked| asked.media == media)
        .map_or(&SERVING_ALONE[..], |asked| asked.forms);
    live::serving::run(&scratch(&arm), media, forms, &one.substitution(Vec::new()));
}

#[test]
fn an_identity_source_standing_in_holds_the_member_whose_request_reaches_a_grab_and_undoes_it() {
    let Some((arm, one, asked)) = standing_in(&[live::IDENTIFIES]) else {
        return;
    };
    let substitution = one.substitution(Vec::new());
    live::run(&scratch(&arm), asked, &live::USENET, Some(&substitution));
}

#[test]
fn a_solver_standing_in_is_told_to_the_indexer_and_fetches_only_pages_outside() {
    let Ok(arm) = std::env::var(ARM) else {
        return;
    };
    let pinned = read::<Pinned>(&root().join("substitutes.toml"));
    assert!(pinned.is_ok(), "{pinned:?}");
    let Ok(pinned) = pinned else {
        return;
    };
    let Some(one) = pinned
        .substitute
        .iter()
        .find(|one| one.arm == arm && one.capability == SOLVES)
    else {
        return;
    };
    live::solving::run(&scratch(&arm), &one.substitution(Vec::new()));
}

/// The directory `arm` runs its stack beneath, emptied of any earlier run's.
fn scratch(arm: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("lemonfiber-arm-{arm}"));
    let _ = std::fs::remove_dir_all(&root);
    root
}

#[test]
fn a_download_client_standing_in_takes_a_household_requests_grab_and_is_paused_and_resumed() {
    let Some((arm, one, asked)) = standing_in(&DOWNLOADS.map(|(capability, _)| capability)) else {
        return;
    };
    let downloads = DOWNLOADS
        .iter()
        .find(|(capability, _)| *capability == one.capability)
        .map(|(_, downloads)| *downloads);
    assert!(downloads.is_some(), "{one:?}");
    let Some(downloads) = downloads else {
        return;
    };
    let substitution = one.substitution(Vec::new());
    live::run(&scratch(&arm), asked, downloads, Some(&substitution));
}
