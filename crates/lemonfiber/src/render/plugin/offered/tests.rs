use super::listed;

use lemonfiber_core::instant::Instant;
use lemonfiber_core::model::{
    CatalogueFailure, CataloguedPlugin, InstalledVersion, PluginCatalogueReport,
    PluginCatalogueState, ReachSetting, SettingWrite,
};

/// A report in `state` carrying nothing else.
fn bare(state: PluginCatalogueState) -> PluginCatalogueReport {
    PluginCatalogueReport {
        state,
        published: None,
        serial: None,
        entries: Vec::new(),
        unreadable: None,
        setting: None,
        failure: None,
    }
}

/// One catalogued plugin, installed at `installed` where that is given.
fn komga(installed: Option<InstalledVersion>) -> CataloguedPlugin {
    CataloguedPlugin {
        id: "komga".to_owned(),
        name: "Komga".to_owned(),
        description: "Reads comics".to_owned(),
        version: "1.3.0".to_owned(),
        without_it: "Comics stay folders of images".to_owned(),
        installed,
    }
}

#[test]
fn a_listing_says_each_plugin_what_is_installed_of_it_and_which_release_it_is() {
    let said = listed(&PluginCatalogueReport {
        published: Some(Instant::from_seconds(1_795_184_656)),
        serial: Some(176),
        entries: vec![
            komga(None),
            komga(Some(InstalledVersion {
                version: "1.2.0".to_owned(),
                newer: true,
            })),
            komga(Some(InstalledVersion {
                version: "1.3.0".to_owned(),
                newer: false,
            })),
        ],
        unreadable: Some(2),
        ..bare(PluginCatalogueState::Listed)
    })
    .text();

    for line in [
        "komga — Komga",
        "does       Reads comics",
        "without    Comics stay folders of images",
        "version    1.3.0",
        "installed  no",
        "installed  1.2.0, and the catalogue holds a newer one",
        "installed  1.3.0\n",
        "2 more could not be listed",
        "Release 176 of the catalogue, published 2026-11-20T14:24:16 UTC.",
    ] {
        assert!(said.contains(line), "{line:?} not in {said}");
    }
}

#[test]
fn a_catalogue_switched_off_names_the_setting_that_allows_it() {
    let said = listed(&PluginCatalogueReport {
        setting: Some(ReachSetting {
            key: "LEMONFIBER_OFFLINE".to_owned(),
            value: "off".to_owned(),
            action: SettingWrite::ConfigSet,
        }),
        ..bare(PluginCatalogueState::Off)
    })
    .text();

    assert!(said.contains("switched off"), "{said}");
    assert!(
        said.contains("lemonfiber config set LEMONFIBER_OFFLINE off"),
        "{said}"
    );
}

#[test]
fn a_catalogue_not_read_says_why_and_lists_nothing() {
    let said = listed(&PluginCatalogueReport {
        failure: Some(CatalogueFailure {
            code: "PLUGIN-19".to_owned(),
            summary: "The catalogue's index could not be read".to_owned(),
        }),
        ..bare(PluginCatalogueState::Failed)
    })
    .text();

    assert!(
        said.contains("could not be read, so nothing is listed"),
        "{said}"
    );
    assert!(said.contains("(PLUGIN-19)"), "{said}");
    assert!(!said.contains("What the plugin catalogue offers"), "{said}");
}

#[test]
fn a_listing_with_nothing_it_can_list_says_so_through_the_outcome() {
    let said = crate::render::shaped(&lemonfiber_core::app::Outcome::PluginCatalogue(
        PluginCatalogueReport {
            unreadable: Some(0),
            ..bare(PluginCatalogueState::Listed)
        },
    ))
    .text();

    assert!(
        said.contains("What the plugin catalogue offers to install:"),
        "{said}"
    );
    assert!(
        said.contains("offers no plugin this build can list"),
        "{said}"
    );
    assert!(!said.contains("could not be listed"), "{said}");
}

#[test]
fn a_catalogue_off_or_failed_with_nothing_more_to_say_says_only_that() {
    for (state, headline) in [
        (
            PluginCatalogueState::Off,
            "Asking the plugin catalogue is switched off, so it was not read.",
        ),
        (
            PluginCatalogueState::Failed,
            "The plugin catalogue could not be read, so nothing is listed.",
        ),
    ] {
        let said = listed(&bare(state)).text();

        assert_eq!(said.trim(), headline, "{said}");
    }
}
