use super::{consequence, cost, declared, refused, switch, switched, CARRIED};
use crate::outbound::OnBehalf;
use crate::plugin::{Register, Stopped, Stopping};

/// One installed plugin whose recipes reach one host outside the stack, one of the
/// stack's own services, and the outside host a second time, with what refusing the
/// outside host stops.
const INSTALLED: &str = r#"{
  "installed": [
    {
      "plugin": "comics",
      "version": "1.2.0",
      "services": [],
      "declared": {
        "reaches": ["metadata.example", "sonarr"],
        "stops": [
          {
            "host": "metadata.example",
            "said": "New series arrive undescribed.",
            "steps": [
              { "recipe": "describe", "title": "Describe new series", "step": "tell" },
              { "recipe": "describe", "title": "Describe new series", "step": "file" }
            ]
          },
          { "host": "sonarr", "steps": [] }
        ]
      }
    }
  ]
}"#;

const SWITCH: &str = "plugin.comics.reach.metadata.example";

fn stack() -> Vec<lemonfiber_manifest::Service> {
    crate::test_support::stack()
        .manifest()
        .map(|manifest| manifest.services)
        .unwrap_or_default()
}

fn listed(recorded: &dyn Fn(&str) -> Option<String>) -> Vec<OnBehalf> {
    Register::parse(INSTALLED)
        .map(|register| declared(&stack(), register.installed(), recorded))
        .unwrap_or_default()
}

#[test]
fn a_host_a_plugins_recipes_declare_is_sent_to_on_its_behalf_and_one_inside_the_stack_is_not() {
    assert_eq!(
        listed(&|_| None),
        vec![OnBehalf {
            plugin: "comics".to_owned(),
            destination: "metadata.example".to_owned(),
            purpose: CARRIED.to_owned(),
            allowed: true,
            switch: SWITCH.to_owned(),
            cost: "New series arrive undescribed. Not asked: Describe new series (tell, file)."
                .to_owned(),
        }]
    );
}

#[test]
fn a_host_is_refused_by_its_own_setting_and_no_other() {
    let off = |name: &str| (name == SWITCH).then(|| "off".to_owned());
    assert_eq!(
        listed(&off)
            .iter()
            .map(|one| one.allowed)
            .collect::<Vec<_>>(),
        vec![false]
    );
    let other =
        |name: &str| (name == "plugin.other.reach.metadata.example").then(|| "off".to_owned());
    assert_eq!(
        listed(&other)
            .iter()
            .map(|one| one.allowed)
            .collect::<Vec<_>>(),
        vec![true]
    );
    let refusing = Register::parse(INSTALLED).ok().and_then(|register| {
        register
            .installed()
            .first()
            .map(|one| refused(&stack(), one, &off))
    });
    assert_eq!(refusing, Some(vec!["metadata.example".to_owned()]));
}

#[test]
fn a_switch_names_its_plugin_and_its_host() {
    assert_eq!(switch("comics", "metadata.example"), SWITCH);
    assert_eq!(switched(SWITCH), Some(("comics", "metadata.example")));
    for not in [
        "plugin..reach.x",
        "plugin.comics.reach.",
        "LEMONFIBER_REACH_POSTERS",
    ] {
        assert_eq!(switched(not), None, "{not}");
    }
}

#[test]
fn the_cost_is_the_steps_alone_where_the_author_said_nothing() {
    let stopping = Stopping {
        host: "metadata.example".to_owned(),
        said: None,
        steps: vec![Stopped {
            recipe: "describe".to_owned(),
            title: "Describe new series".to_owned(),
            step: "tell".to_owned(),
        }],
    };
    assert_eq!(
        cost(Some(&stopping)),
        "Not asked: Describe new series (tell)."
    );
    assert!(cost(None).contains("was not recorded"));
}

#[test]
fn a_host_no_step_calls_costs_no_step() {
    let stopping = Stopping {
        host: "metadata.example".to_owned(),
        said: None,
        steps: Vec::new(),
    };
    assert_eq!(
        cost(Some(&stopping)),
        "No step of its recipes calls it, so refusing it stops none."
    );
}

#[test]
fn switching_a_reach_says_what_refusing_stops_and_that_allowing_asks_again() {
    let installed = Register::parse(INSTALLED)
        .map(|register| register.installed().to_vec())
        .unwrap_or_default();
    assert_eq!(
        consequence(&installed, SWITCH, Some("off")).as_deref(),
        Some("New series arrive undescribed. Not asked: Describe new series (tell, file).")
    );
    assert_eq!(
        consequence(&installed, SWITCH, Some("on")).as_deref(),
        Some("comics's recipes may ask metadata.example again.")
    );
    assert_eq!(
        consequence(
            &installed,
            "plugin.comics.reach.elsewhere.example",
            Some("off")
        ),
        None
    );
    assert_eq!(
        consequence(
            &installed,
            "plugin.other.reach.metadata.example",
            Some("off")
        ),
        None
    );
    assert_eq!(
        consequence(&installed, "LEMONFIBER_REACH", Some("off")),
        None
    );
}

#[test]
fn nothing_installed_is_sent_to_on_nobodys_behalf() {
    assert!(declared(&stack(), &[], &|_| None).is_empty());
}
