//! What browsing the plugin catalogue answers with, in each of its three states.

use lemonfiber_core::instant::Instant;
use lemonfiber_core::model::{
    CatalogueFailure, CataloguedPlugin, InstalledVersion, PluginCatalogueReport,
    PluginCatalogueState, ReachSetting, SettingWrite,
};

/// A verified index offering one plugin, installed here at an older version.
pub(crate) fn listed() -> PluginCatalogueReport {
    PluginCatalogueReport {
        state: PluginCatalogueState::Listed,
        published: Some(Instant::from_seconds(1_795_184_656)),
        serial: Some(176),
        entries: vec![CataloguedPlugin {
            id: "komga".to_owned(),
            name: "Komga".to_owned(),
            description: "Reads comics, manga and magazines in a browser".to_owned(),
            version: "1.1.0".to_owned(),
            without_it: "Comics stay folders of images".to_owned(),
            installed: Some(InstalledVersion {
                version: "1.0.0".to_owned(),
                newer: true,
            }),
        }],
        unreadable: Some(0),
        setting: None,
        failure: None,
    }
}

/// Reaching the catalogue switched off.
pub(crate) fn off() -> PluginCatalogueReport {
    PluginCatalogueReport {
        state: PluginCatalogueState::Off,
        published: None,
        serial: None,
        entries: Vec::new(),
        unreadable: None,
        setting: Some(ReachSetting {
            key: "LEMONFIBER_REACH_CATALOGUE".to_owned(),
            value: "on".to_owned(),
            action: SettingWrite::ConfigSet,
        }),
        failure: None,
    }
}

/// An index that could not be fetched.
pub(crate) fn failed() -> PluginCatalogueReport {
    PluginCatalogueReport {
        state: PluginCatalogueState::Failed,
        published: None,
        serial: None,
        entries: Vec::new(),
        unreadable: None,
        setting: None,
        failure: Some(CatalogueFailure {
            code: "PLUGIN-19".to_owned(),
            summary: "The catalogue's index could not be read".to_owned(),
        }),
    }
}
