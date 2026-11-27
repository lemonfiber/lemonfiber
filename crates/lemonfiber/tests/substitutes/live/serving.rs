//! A media server standing in, proved through what it serves: the library the seed
//! furnished it holds a title written there once the walkthrough has it look again, and,
//! where it holds the household's members too, a member invited into it holds that title.
//! One claimed against an account outside the house is installed and left unclaimed, and
//! read as held by nobody.

use std::path::Path;
use std::time::Duration;

use lemonfiber_contract::capabilities::owner::claim;
use lemonfiber_error::codes::config::PART_UNCLAIMED;

use super::core::{strings_under, Core};
use super::{
    fixture, providing, started, step, undone, waited_every, Substitution, IDENTIFIES, LOOKING,
    MEMBER,
};

/// What setup is told to fetch over: nothing, which makes it a stack serving a library
/// alone.
const NOTHING_FETCHED: &str = "none";

/// The state a wiring that did not come to what it should is in.
const FAILED: &str = "failed";

/// How long a title written into a library may take to be held.
const HELD_WITHIN: Duration = Duration::from_secs(5 * 60);

/// Whether what `core` reads as held, for `whom`, names the fixture.
fn holds(core: &Core, whom: &[&str]) -> bool {
    let mut asked = vec!["held"];
    asked.extend(whom);
    strings_under(&core.run(&asked, "").said, "title")
        .iter()
        .any(|title| title == fixture::TITLE)
}

/// Run `substitution`, a media server, live through a stack this build sets up beneath
/// `root` with `forms`, serving `media`, chosen as the identity source too where it is
/// one, since installing it contests the stack's.
///
/// # Panics
///
/// At the first step that does not come to what it should, naming it and what was said.
pub fn run(root: &Path, media: &str, forms: &[&str], substitution: &Substitution) {
    let identifies = providing(&substitution.plugin, IDENTIFIES).is_some();
    let substitution = Substitution {
        also: identifies
            .then(|| IDENTIFIES.to_owned())
            .into_iter()
            .collect(),
        ..substitution.clone()
    };
    let Some(core) = started(root, forms, (NOTHING_FETCHED, &[]), Some(&substitution)) else {
        return;
    };

    let library = super::library(root, media);
    assert!(
        library.is_dir(),
        "the seed furnished no library for {media} at {}",
        library.display()
    );
    let written = fixture::written(&library, media);
    assert!(written.is_ok(), "{written:?}");
    assert!(
        waited_every(HELD_WITHIN, LOOKING, || {
            let _ = core.run(&["walkthrough", fixture::TITLE], "");
            holds(&core, &["--defaults"])
        }),
        "{} never held {written:?}",
        substitution.plugin
    );

    if identifies {
        step(&core.run(&["invite", MEMBER], ""), "invite");
        assert!(
            waited_every(HELD_WITHIN, LOOKING, || holds(&core, &["--member", MEMBER])),
            "{MEMBER} never held {written:?} in {}",
            substitution.plugin
        );
    }

    undone(&core);
}

/// What `doctor` summarises a part nobody holds as, for the part called `name`.
fn held_by_nobody(name: &str) -> String {
    format!("Nobody holds {name}")
}

/// The name of the part the plugin `tree` holds claims over `owner.claim`: the upstream its
/// speaker fronts, or the speaker itself.
///
/// # Errors
///
/// Where its manifest does not read, or no service of it speaks `owner.claim`.
fn claimed_part(tree: &Path) -> Result<String, String> {
    let at = tree.join("plugin.toml");
    let written = std::fs::read_to_string(&at).map_err(|why| format!("{}: {why}", at.display()))?;
    let manifest: toml::Value =
        toml::from_str(&written).map_err(|why| format!("{}: {why}", at.display()))?;
    let services = manifest
        .get("service")
        .and_then(toml::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let spoken = lemonfiber_contract::spoken(claim::CAPABILITY, claim::MAJOR);
    let speaker = services
        .iter()
        .find(|one| {
            one.get("speaks")
                .and_then(toml::Value::as_array)
                .is_some_and(|speaks| speaks.iter().any(|one| one.as_str() == Some(&spoken)))
        })
        .ok_or_else(|| format!("no service of {} speaks {spoken}", at.display()))?;
    let part = speaker
        .get("fronts")
        .and_then(toml::Value::as_str)
        .and_then(|fronted| {
            services
                .iter()
                .find(|one| one.get("id").and_then(toml::Value::as_str) == Some(fronted))
        })
        .unwrap_or(speaker);
    part.get("name")
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("the part {} claims is not named", at.display()))
}

/// Every document of `said`: each one a run printed, or the one it printed.
fn documents(said: &serde_json::Value) -> Vec<&serde_json::Value> {
    match said {
        serde_json::Value::Array(each) => each.iter().collect(),
        one => vec![one],
    }
}

/// Install the media server the plugin checked out at `tree` holds, whose upstream is
/// claimed against an account outside the house, into a stack this build sets up beneath
/// `root` with `forms`, seed it with no claim code given, and read it as held by nobody.
///
/// # Panics
///
/// At the first step that does not come to what it should, naming it and what was said.
pub fn unclaimed(root: &Path, forms: &[&str], tree: &Path) {
    let name = claimed_part(tree);
    assert!(name.is_ok(), "{name:?}");
    let Ok(name) = name else {
        return;
    };
    let Some(core) = started(root, forms, (NOTHING_FETCHED, &[]), None) else {
        return;
    };
    step(
        &core.agreed(&["plugin", "install", &tree.to_string_lossy()]),
        "install",
    );
    let seeded = core.run(&["seed"], "");
    let failed: Vec<&serde_json::Value> = documents(&seeded.said)
        .into_iter()
        .filter_map(|one| {
            one.pointer("/data/wirings")
                .and_then(serde_json::Value::as_array)
        })
        .flatten()
        .filter(|wiring| {
            wiring
                .get("connection")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|connection| connection.starts_with(&name))
                && wiring
                    .pointer("/state/state")
                    .and_then(serde_json::Value::as_str)
                    == Some(FAILED)
        })
        .collect();
    assert!(
        failed.is_empty(),
        "seeding {name} unclaimed failed: {failed:?}"
    );
    let read = core.run(&["doctor"], "");
    let nobody = documents(&read.said)
        .into_iter()
        .filter_map(|one| {
            one.pointer("/data/findings")
                .and_then(serde_json::Value::as_array)
        })
        .flatten()
        .any(|finding| {
            finding
                .pointer("/verdict/code")
                .and_then(serde_json::Value::as_str)
                == Some(PART_UNCLAIMED.as_str())
                && finding
                    .pointer("/verdict/summary")
                    .and_then(serde_json::Value::as_str)
                    == Some(held_by_nobody(&name).as_str())
        });
    assert!(
        nobody,
        "doctor does not read {name} as held by nobody:\n{}",
        read.printed
    );
}
