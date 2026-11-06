use serde_norway::Value as Yaml;

use super::{carried, entry, environment};
use crate::certificate::fronted::{AUTHORITY, HOSTS};
use crate::plex_link::{SETTING, TRUSTED};
use crate::plugin::Installed;
use crate::project::{generated, Project, Reading};
use crate::wiring::Chosen;

/// Plex moved into its plugin, beside the request gate asking for it in Plex's API and the
/// request service moved into its plugin.
fn beside_plex() -> Vec<Installed> {
    crate::test_support::plexing::plexed()
        .into_iter()
        .chain(
            crate::test_support::guards::GATE
                .installed()
                .map(crate::test_support::plexing::asking_in_plex_too),
        )
        .chain(crate::test_support::requests::REQUESTS.installed())
        .collect()
}

/// The project beside Plex with `mode` recorded as the link mode, where it names one.
fn written(mode: Option<&'static str>) -> Project {
    let setting = move |name: &str| mode.filter(|_| name == SETTING).map(str::to_owned);
    generated(
        &carried(),
        &beside_plex(),
        &Chosen::read(None),
        &Reading {
            setting: &setting,
            ..Reading::bare()
        },
    )
}

/// Every name `entry` answers to on any network beyond its own.
fn answers(entry: &Yaml) -> Vec<String> {
    entry
        .get("networks")
        .and_then(Yaml::as_mapping)
        .into_iter()
        .flatten()
        .filter_map(|(_, held)| held.get("aliases")?.as_sequence())
        .flatten()
        .filter_map(|alias| alias.as_str().map(str::to_owned))
        .collect()
}

#[test]
fn fronted_has_the_gate_answer_to_plex_tv_and_the_request_service_trust_its_authority() {
    let project = written(Some("fronted"));

    let gate = answers(&entry(&project, crate::app::gating::SERVICE));
    for host in HOSTS {
        assert!(gate.iter().any(|alias| alias == host), "{host}: {gate:?}");
    }
    let trusted = environment(&entry(&project, "seerr"), TRUSTED);
    assert!(trusted.ends_with(&format!("/{AUTHORITY}")), "{trusted}");
    assert!(trusted.starts_with('/'), "{trusted}");
}

#[test]
fn every_other_mode_writes_neither() {
    for mode in [None, Some("managed"), Some("owner")] {
        let project = written(mode);

        let gate = answers(&entry(&project, crate::app::gating::SERVICE));
        assert!(
            HOSTS
                .iter()
                .all(|host| !gate.iter().any(|alias| alias == host)),
            "{mode:?}: {gate:?}"
        );
        assert_eq!(
            environment(&entry(&project, "seerr"), TRUSTED),
            "",
            "{mode:?}"
        );
    }
}
