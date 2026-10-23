use super::{declared, CARRIED};
use crate::outbound::OnBehalf;
use crate::plugin::Register;

/// One installed plugin whose recipes reach one host outside the stack, one of the
/// stack's own services, and the outside host a second time.
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

fn stack() -> Vec<lemonfiber_manifest::Service> {
    crate::test_support::stack()
        .manifest()
        .map(|manifest| manifest.services)
        .unwrap_or_default()
}

#[test]
fn a_host_a_plugins_recipes_declare_is_sent_to_on_its_behalf_and_one_inside_the_stack_is_not() {
    let listed = Register::parse(INSTALLED)
        .map(|register| declared(&stack(), register.installed()))
        .unwrap_or_default();

    assert_eq!(
        listed,
        vec![OnBehalf {
            plugin: "comics".to_owned(),
            destination: "metadata.example".to_owned(),
            purpose: CARRIED.to_owned(),
        }]
    );
}

#[test]
fn nothing_installed_is_sent_to_on_nobodys_behalf() {
    assert!(declared(&stack(), &[]).is_empty());
}
