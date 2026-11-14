//! A plugin's reach to one host is a setting of its own: known only where an installed
//! plugin reaches the host, taking `on` or `off`, and said to be the plugin's.

use super::changing::{asked, change, machine, running};
use crate::model::Owner;
use crate::plugin::Register;
use crate::settings::fences::valued;
use crate::settings::readers::{readers, whose};

const INSTALLED: &str = r#"{
  "installed": [
    {
      "plugin": "comics",
      "version": "1.2.0",
      "services": [],
      "declared": { "reaches": ["metadata.example", "sonarr"] }
    }
  ]
}"#;

const SWITCH: &str = "plugin.comics.reach.metadata.example";

#[test]
fn each_host_an_installed_plugin_reaches_outside_the_stack_is_a_setting_with_two_choices() {
    let read = crate::test_support::stack()
        .manifest()
        .ok()
        .and_then(|manifest| {
            Register::parse(INSTALLED)
                .ok()
                .map(|register| readers(&manifest, register.installed(), &|_| None))
        });
    assert!(read
        .as_ref()
        .is_some_and(|read| read.settings.contains_key(SWITCH)
            && !read.settings.contains_key("plugin.comics.reach.sonarr")));
    assert_eq!(
        read.and_then(|read| read.choices.get(SWITCH).cloned())
            .map(|choices| choices.into_iter().map(|one| one.value).collect::<Vec<_>>()),
        Some(vec!["on".to_owned(), "off".to_owned()])
    );
}

#[test]
fn a_reach_setting_takes_on_or_off_and_nothing_else() {
    assert!(valued(SWITCH, "off").is_ok());
    assert!(valued(SWITCH, "on").is_ok());
    let refused = valued(SWITCH, "maybe").err();
    assert!(refused.is_some_and(|problem| problem.meaning.contains("on or off")));
}

#[test]
fn a_reach_setting_is_its_plugins() {
    assert_eq!(
        whose(SWITCH),
        Owner::Namespace {
            namespace: "comics".to_owned()
        }
    );
}

#[test]
fn forgetting_a_plugin_takes_its_reach_settings_and_no_other_plugins() {
    let scratch = lemonfiber_fixtures::scratch::Scratch::new("reach-forgotten");
    let path = scratch.path().join("settings.json");
    let changing = std::collections::BTreeMap::from([
        (SWITCH, Some("off")),
        ("plugin.comicstwo.reach.metadata.example", Some("off")),
    ]);
    assert!(crate::settings::store::recorded(&path, &changing).is_ok());
    assert!(crate::settings::store::forgotten(&path, "comics").is_ok());
    let kept = crate::settings::store::Settings::at(&path).ok();
    assert_eq!(kept.as_ref().and_then(|kept| kept.value(SWITCH)), None);
    assert_eq!(
        kept.as_ref()
            .and_then(|kept| kept.value("plugin.comicstwo.reach.metadata.example")),
        Some("off")
    );
}

#[tokio::test]
async fn refusing_a_reach_says_what_refusing_it_stops_before_it_lands() {
    let machine = machine("reach-consequence", running(&[]));
    let register = Register::parse(INSTALLED).unwrap_or_else(|_| Register::empty());
    let kept = crate::app::targets::beside_env(&machine.ctx, crate::config::paths::PLUGINS);
    assert!(crate::app::record::keep(kept.as_deref(), &register).is_ok());

    let offered = change(asked(&machine.ctx, SWITCH, Some("off"), None).await);

    let consequences: Vec<Option<&str>> = offered
        .iter()
        .flat_map(|change| &change.settings)
        .map(|one| one.consequence.as_deref())
        .collect();
    assert!(
        matches!(consequences.as_slice(), [Some(said)] if said.contains("was not recorded")),
        "{consequences:?}"
    );
}
