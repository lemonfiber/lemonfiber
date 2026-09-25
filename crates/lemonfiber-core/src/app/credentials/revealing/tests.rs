use super::reveal;
use crate::app::targets::record_secret;
use crate::config::Settings;
use crate::credential::{Held, Origin, State, REVEALED, SHOULDER};
use crate::test_support::a_context;

/// An inventory line naming a credential recorded under a test's own setting.
fn line(setting: &str) -> Held {
    Held {
        name: "qBittorrent web UI password".to_owned(),
        setting: setting.to_owned(),
        consumers: vec!["qBittorrent".to_owned()],
        location: "the settings file".to_owned(),
        origin: Origin::Lemonfiber,
        from: crate::origin::Origin::Bundled,
        state: State::Active,
        fingerprint: None,
        advisory: None,
    }
}

/// A run whose settings file is a scratch path unique to the named test, so
/// concurrent tests do not share one.
fn keeping(name: &str) -> crate::app::Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("reveal-{name}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    a_context()
        .settings(Settings {
            env_file: Some(dir.join(".env")),
            ..Settings::default()
        })
        .build()
}

/// A value built rather than written, so nothing reads it as a real credential.
fn a_value() -> String {
    format!("{}{}", "the-", "recorded-value")
}

#[test]
fn asking_once_prints_the_warning_and_not_the_value() {
    let ctx = keeping("once");
    let secret = a_value();
    record_secret(&ctx, "A_TEST_PASS", &secret);

    let revealed = reveal(&ctx, &line("A_TEST_PASS"), false);

    assert_eq!(revealed.value, None);
    assert_eq!(revealed.warning, SHOULDER);
}

#[test]
fn asking_again_prints_the_value_with_the_warning_still_beside_it() {
    let ctx = keeping("again");
    let secret = a_value();
    record_secret(&ctx, "A_TEST_PASS", &secret);

    let revealed = reveal(&ctx, &line("A_TEST_PASS"), true);

    assert_eq!(revealed.value.as_deref(), Some(secret.as_str()));
    assert_eq!(revealed.warning, REVEALED);
}

/// Even where a value happens to sit under the same name, because a plugin's
/// secret is not one lemonfiber has captured and anything found there is not it.
#[test]
fn a_plugins_secret_says_nothing_holds_it_rather_than_showing_anything() {
    let ctx = keeping("plugin");
    record_secret(&ctx, "comics/api_key", &a_value());
    let mut held = line("comics/api_key");
    held.from = crate::origin::Origin::Plugin {
        named: "comics".to_owned(),
    };

    let revealed = reveal(&ctx, &held, true);

    assert_eq!(revealed.value, None);
    assert!(
        revealed.warning.contains("the plugin comics's"),
        "{}",
        revealed.warning
    );
}

#[test]
fn a_credential_with_nothing_recorded_says_so_rather_than_showing_an_empty_value() {
    let ctx = keeping("nothing");

    let revealed = reveal(&ctx, &line("NOTHING_RECORDED_PASS"), true);

    assert_eq!(revealed.value, None);
    assert!(
        revealed.warning.contains("NOTHING_RECORDED_PASS"),
        "{}",
        revealed.warning
    );
}
