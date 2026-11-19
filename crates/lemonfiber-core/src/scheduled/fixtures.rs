//! Installed plugins holding run-and-exit services, for the tests that read them.

use lemonfiber_manifest::container::Fields;
use lemonfiber_manifest::schedule::Schedule;

use crate::plugin::{Installed, Placed};

/// The setting the services here read their times from.
pub(crate) const SETTING: &str = "configarr:schedule";

/// One plugin holding a service for each of `services`, scheduled at the default
/// given beside it or kept running where none is.
pub(crate) fn installed(services: &[(&str, Option<&str>)]) -> Vec<Installed> {
    vec![Installed {
        granted: Vec::new(),
        plugin: "configarr".to_owned(),
        version: "1.0.0".to_owned(),
        name: None,
        description: None,
        services: services
            .iter()
            .map(|(service, default)| placed(service, *default))
            .collect(),
        provides: Vec::new(),
        contributions: Vec::new(),
        run_proofs: Vec::new(),
        request_proofs: Vec::new(),
        declared: crate::plugin::Declaration::default(),
        recipes: Vec::new(),
        from: String::new(),
        revision: String::new(),
        signed: String::new(),
        manifest: String::new(),
        tree: String::new(),
        installed_at: None,
        forms: Vec::new(),
    }]
}

/// A service scheduled at `default`, or kept running where that is none.
fn placed(service: &str, default: Option<&str>) -> Placed {
    Placed {
        fields: Fields {
            schedule: default.map(|default| Schedule {
                setting: SETTING.to_owned(),
                default: default.to_owned(),
            }),
            ..Fields::default()
        },
        service: service.to_owned(),
        image: "example.invalid/configarr".to_owned(),
        digest: "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
            .to_owned(),
        tag: "1.0.0".to_owned(),
        built_on: None,
        config_path: "/config".to_owned(),
        takes_data: false,
        reached: None,
        provides: Vec::new(),
        name: service.to_owned(),
        description: String::new(),
        icon: None,
        hidden: false,
        health: None,
        listens: None,
        contract: None,
        media_types: Vec::new(),
        networks: Vec::new(),
        speaks: Vec::new(),
        fronts: None,
        upstream: std::collections::BTreeMap::new(),
        native: None,
        replaces: None,
        shape: None,
        asks: Vec::new(),
        reaches: None,
        asks_for: None,
        starting: None,
    }
}
