//! A plugin whose services sit in a profile of its own joins the closure of each form it
//! names, and a form the stack no longer declares is named in the plan.

use lemonfiber_manifest::Manifest;

use super::super::{everything, joined, resolve, Joining, Plan, Protocols, Vanished};
use super::{named, STACK};

/// A plugin bringing one service, joining `forms`.
fn plugin(id: &str, forms: &[&str]) -> Joining {
    Joining {
        plugin: id.to_owned(),
        profile: format!("plugin-{id}"),
        services: vec![id.to_owned()],
        forms: named(forms),
    }
}

/// `forms` of the committed stack resolved, with `plugins` joined, or everything where
/// none is named.
fn planned(forms: &[&str], plugins: &[Joining]) -> Option<Plan> {
    let manifest = Manifest::from_toml(STACK).ok()?;
    let plan = if forms.is_empty() {
        everything(&manifest, Protocols::both())
    } else {
        resolve(&manifest, &named(forms), Protocols::both())
    }
    .ok()?;
    Some(joined(plan, &manifest, plugins))
}

#[test]
fn a_plugins_profile_joins_the_closure_of_each_form_it_names() {
    let comics = [plugin("komga", &["books", "full"])];
    for form in ["books", "full"] {
        let plan = planned(&[form], &comics);
        assert!(plan
            .as_ref()
            .is_some_and(|plan| plan.profiles.contains("plugin-komga")));
        assert!(plan.is_some_and(|plan| plan.services.last().map(String::as_str) == Some("komga")));
    }
}

#[test]
fn a_plugin_none_of_whose_forms_is_named_does_not_run() {
    let plan = planned(&["tv"], &[plugin("komga", &["books"])]);
    assert!(plan
        .as_ref()
        .is_some_and(|plan| !plan.profiles.contains("plugin-komga")
            && !plan.services.iter().any(|service| service == "komga")));
    assert!(plan.is_some_and(|plan| plan.vanished.is_empty()));
}

#[test]
fn naming_no_form_runs_every_plugin_naming_a_declared_form() {
    let plan = planned(
        &[],
        &[plugin("komga", &["books"]), plugin("ghost", &["cartoons"])],
    );
    assert!(plan
        .as_ref()
        .is_some_and(|plan| plan.profiles.contains("plugin-komga")));
    assert!(plan.is_some_and(|plan| !plan.profiles.contains("plugin-ghost")));
}

#[test]
fn a_form_the_stack_no_longer_declares_is_named_with_its_plugin() {
    let plan = planned(
        &["books"],
        &[
            plugin("komga", &["books", "comics"]),
            plugin("ghost", &["cartoons"]),
        ],
    );
    assert_eq!(
        plan.as_ref().map(|plan| plan.vanished.clone()),
        Some(vec![
            Vanished {
                plugin: "komga".to_owned(),
                forms: named(&["comics"]),
            },
            Vanished {
                plugin: "ghost".to_owned(),
                forms: named(&["cartoons"]),
            },
        ])
    );
    assert!(plan
        .as_ref()
        .is_some_and(|plan| plan.profiles.contains("plugin-komga")));
    assert!(plan.is_some_and(|plan| !plan.profiles.contains("plugin-ghost")));
}

#[test]
fn a_plugin_whose_record_kept_no_forms_joins_every_form() {
    let plan = planned(&["tv"], &[plugin("older", &[])]);
    assert!(
        plan.is_some_and(|plan| plan.profiles.contains("plugin-older") && plan.vanished.is_empty())
    );
}

#[test]
fn a_plugin_runs_with_the_forms_the_last_start_ran() {
    let manifest = Manifest::from_toml(STACK).ok();
    let comics = plugin("komga", &["books"]);
    assert!(manifest.as_ref().is_some_and(|manifest| comics
        .runs_with(&named(&["books", "tv"]), manifest)
        && !comics.runs_with(&named(&["tv"]), manifest)
        && comics.runs_with(&[], manifest)));
}
