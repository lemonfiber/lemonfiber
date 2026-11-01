use lemonfiber_manifest::container::{Constant, Setting, Value};

use crate::test_support::{a_placed, an_installed};

fn keeping(plugin: &str, setting: &str, was: &str, secret: Option<&str>) -> super::Installed {
    let mut placed = a_placed(plugin, &[], None);
    placed.fields.env.insert(
        "LEVEL".to_owned(),
        Value::Setting(Setting {
            setting: setting.to_owned(),
            default: Constant::Text(String::new()),
            recorded_with: None,
            was: Some(was.to_owned()),
            needs: lemonfiber_manifest::container::choices::Needs::new(),
            one_of: None,
        }),
    );
    let mut installed = an_installed(plugin, vec![placed]);
    installed.declared.secrets = secret
        .map(|key| crate::plugin::Secret {
            id: "key".to_owned(),
            of: plugin.to_owned(),
            why: "Signs in".to_owned(),
            minted: false,
            serves: None,
            was: Some(key.to_owned()),
            needs: lemonfiber_manifest::container::choices::Needs::new(),
            needed_to_start: false,
            lapses_s: None,
            captured: false,
        })
        .into_iter()
        .collect();
    installed
}

#[test]
fn a_plugin_names_what_its_settings_and_secrets_were_kept_under() {
    let named: Vec<(String, String, bool)> =
        super::of(&keeping("one", "one:level", "ONE_LEVEL", Some("ONE_KEY")))
            .into_iter()
            .map(|earlier| (earlier.key, earlier.name, earlier.secret))
            .collect();
    assert_eq!(
        named,
        vec![
            ("ONE_LEVEL".to_owned(), "one:level".to_owned(), false),
            ("ONE_KEY".to_owned(), "one:key".to_owned(), true),
        ]
    );
}

#[test]
fn a_variable_another_plugin_names_is_taken_and_its_own_earlier_record_is_not() {
    let held = keeping("one", "one:level", "LOG_LEVEL", None);
    let would = keeping("two", "two:level", "TWO_LEVEL", Some("LOG_LEVEL"));
    let taken = super::taken(&would, std::slice::from_ref(&held));
    assert_eq!(
        taken.map(|(ours, theirs, plugin)| (ours.named(), theirs.named(), plugin)),
        Some((
            "secret two:key".to_owned(),
            "setting one:level".to_owned(),
            "one".to_owned()
        ))
    );
    let again = keeping("one", "one:level", "LOG_LEVEL", None);
    assert_eq!(super::taken(&again, &[held]), None);
    assert_eq!(
        super::taken(&keeping("two", "two:level", "TWO_LEVEL", None), &[again]),
        None
    );
}
