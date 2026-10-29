//! A part a choice of filler passed over is written out of its form.

use serde_norway::Value as Yaml;

use super::{carried, entry};
use crate::project::{generated, Reading, REPLACED};
use crate::wiring::Chosen;

fn profiles(entry: &Yaml) -> Vec<String> {
    entry
        .get("profiles")
        .and_then(Yaml::as_sequence)
        .map(|profiles| {
            profiles
                .iter()
                .filter_map(|one| one.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_part_passed_over_is_written_under_a_profile_no_form_starts() {
    let manifest = carried();
    let chosen = Chosen::read(Some("indexer.search=nzbhydra2"));

    let project = generated(&manifest, &[], &chosen, &Reading::bare());

    assert_eq!(profiles(&entry(&project, "prowlarr")), vec![REPLACED]);
    assert_ne!(profiles(&entry(&project, "nzbhydra2")), vec![REPLACED]);
    assert!(!entry(&project, "prowlarr").is_null());
}
