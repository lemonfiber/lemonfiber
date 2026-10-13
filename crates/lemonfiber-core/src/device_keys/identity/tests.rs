use super::{read_all, trusted, AppIdentity, OFFICIAL, SETTING};

/// A digest written in hex.
const DIGEST: &str = "ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12";

#[test]
fn each_form_reads_and_is_written_back_as_it_was_read() {
    for written in [
        format!("android:app.lemonfiber.companion@{DIGEST}"),
        "ios:ABCDE12345.app.lemonfiber.companion".to_owned(),
    ] {
        assert_eq!(
            AppIdentity::read(&written).map(|identity| identity.written()),
            Some(written)
        );
    }
}

#[test]
fn a_digest_in_capitals_is_kept_in_lower_case() {
    let read = AppIdentity::read(&format!(
        "android:app.lemonfiber.companion@{}",
        DIGEST.to_uppercase()
    ));
    assert_eq!(
        read.map(|identity| identity.written()),
        Some(format!("android:app.lemonfiber.companion@{DIGEST}"))
    );
}

#[test]
fn a_word_in_neither_form_names_nothing() {
    for wrong in [
        "",
        "android:",
        "android:companion@ab",
        &format!("android:companion@{DIGEST}"),
        &format!("android:app.1companion@{DIGEST}"),
        "android:app.lemonfiber.companion@nothex",
        "ios:abcde12345.app",
        "ios:ABCDE1234.app",
        "ios:ABCDE12345.",
        "ios:ABCDE12345.app/companion",
        "windows:anything",
    ] {
        assert_eq!(AppIdentity::read(wrong), None, "{wrong}");
    }
}

#[test]
fn only_an_ios_identity_has_an_app_id() {
    let ios = AppIdentity::read("ios:ABCDE12345.app.lemonfiber.companion");
    assert_eq!(
        ios.and_then(|identity| identity.app_id()),
        Some("ABCDE12345.app.lemonfiber.companion".to_owned())
    );
    let android = AppIdentity::read(&format!("android:app.lemonfiber.companion@{DIGEST}"));
    assert_eq!(android.and_then(|identity| identity.app_id()), None);
}

#[test]
fn a_value_is_read_into_what_it_names_and_what_it_does_not() {
    let (named, wrong) = read_all(&format!(
        "ios:ABCDE12345.app.one , android:app.two@{DIGEST},nonsense"
    ));
    assert_eq!(named.len(), 2);
    assert_eq!(wrong, vec!["nonsense".to_owned()]);
}

#[test]
fn the_official_list_is_empty_until_the_first_store_release() {
    assert!(OFFICIAL.is_empty());
}

#[test]
fn what_is_trusted_is_the_operators_builds_as_the_settings_store_holds_them() {
    let path = lemonfiber_fixtures::scratch::Scratch::new("trusted")
        .within(crate::config::paths::SETTINGS);
    assert!(trusted(Some(&path)).is_ok_and(|apps| apps.is_empty()));
    assert!(trusted(None).is_ok_and(|apps| apps.is_empty()));
    let value = format!("ios:ABCDE12345.app.one,android:app.two@{DIGEST}");
    let written = format!(
        "{{\"schema\":1,\"written_by\":\"0.0.0\",\"values\":{{\"{SETTING}\":\"{value}\"}}}}"
    );
    assert!(std::fs::write(&path, written).is_ok());
    assert_eq!(
        trusted(Some(&path))
            .unwrap_or_default()
            .iter()
            .map(AppIdentity::written)
            .collect::<Vec<_>>(),
        vec![
            "ios:ABCDE12345.app.one".to_owned(),
            format!("android:app.two@{DIGEST}")
        ]
    );
    assert!(std::fs::write(&path, "not a settings store").is_ok());
    assert!(trusted(Some(&path)).is_err());
}
