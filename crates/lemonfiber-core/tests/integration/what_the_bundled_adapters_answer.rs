//! Every first-party adapter the bundle installs, held to the suite a third-party one is:
//! each case of each contract it speaks judged from the recordings its tree ships.

use lemonfiber_bundled::{Pinned, Stage, PINNED};
use lemonfiber_core::plugin::{conformed, Conformed};
use lemonfiber_fixtures::scratch::Scratch;

/// The plugin's tree as the bundle pins it, written out where `conformed` reads a source.
fn written(pinned: &Pinned) -> Scratch {
    let root = Scratch::new(&format!("bundled-adapter-{}", pinned.first_party.plugin));
    let files = pinned
        .files
        .iter()
        .copied()
        .chain([(lemonfiber_bundle::MANIFEST, pinned.copy.as_bytes())]);
    for (path, bytes) in files {
        let at = root.join(path);
        if let Some(parent) = at.parent() {
            std::fs::create_dir_all(parent).unwrap_or_default();
        }
        std::fs::write(&at, bytes).unwrap_or_default();
    }
    root
}

/// Each case that did not pass, as `plugin contract/case: verdict`.
fn unpassed(plugin: &str, read: &Conformed) -> Vec<String> {
    read.refusals
        .iter()
        .map(|refused| format!("{plugin} {}: {}", refused.location, refused.message))
        .chain(
            read.cases
                .iter()
                .filter(|judged| judged.verdict != lemonfiber_core::plugin::Verdict::Passed)
                .map(|judged| {
                    format!(
                        "{plugin} {}/{}: {:?}",
                        judged.contract, judged.case, judged.verdict
                    )
                }),
        )
        .collect()
}

#[tokio::test]
async fn every_first_party_adapter_answers_every_case_of_every_contract_it_speaks() {
    let installed: Vec<&Pinned> = PINNED
        .iter()
        .filter(|pinned| pinned.stage == Some(Stage::Installed))
        .collect();
    let mut judged = 0;
    let mut faults = Vec::new();
    for pinned in &installed {
        let root = written(pinned);
        let read = conformed(&root).await;
        let Ok(read) = read else {
            faults.push(format!("{}: {read:?}", pinned.first_party.plugin));
            continue;
        };
        judged += read.cases.len();
        faults.extend(unpassed(pinned.first_party.plugin, &read));
    }

    assert!(faults.is_empty(), "{}", faults.join("\n"));
    assert!(
        !installed.is_empty() && judged > 0,
        "the bundle installs no adapter"
    );
}
