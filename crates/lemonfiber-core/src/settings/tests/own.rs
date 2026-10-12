use crate::settings::own::{is_configs, is_core, refusal, DEVICE_KEY_APPS};

fn fingerprint() -> String {
    "0f".repeat(32)
}

#[test]
fn the_core_settings_are_a_closed_list_and_every_other_own_key_is_configs() {
    assert!(is_core(DEVICE_KEY_APPS));
    assert!(!is_configs(DEVICE_KEY_APPS));
    assert!(is_configs("LEMONFIBER_IDEMPOTENCY_MINUTES"));
    assert!(!is_core("LEMONFIBER_IDEMPOTENCY_MINUTES"));
    assert!(!is_configs("VPN_COUNTRIES"));
    assert!(!is_core("gluetun:countries"));
}

#[test]
fn an_identity_in_either_form_is_taken() {
    for value in [
        format!("android:app.example.lemon@{}", fingerprint()),
        format!("android:a.b_c@{}", fingerprint().to_uppercase()),
        "ios:ABCDE12345.app.example.lemon".to_owned(),
        format!(
            "android:app.example.lemon@{}, ios:ABCDE12345.app-example",
            fingerprint()
        ),
    ] {
        assert_eq!(refusal(DEVICE_KEY_APPS, &value), None, "{value}");
    }
}

#[test]
fn an_identity_in_neither_form_is_refused_naming_it() {
    for wrong in [
        "android:app@0f".to_owned(),
        format!("android:app@{}", fingerprint()),
        format!("android:1app.example@{}", fingerprint()),
        format!("android:app.example@{}", "0g".repeat(32)),
        format!("android:app.example{}", fingerprint()),
        "ios:abcde12345.app".to_owned(),
        "ios:ABCDE1234.app".to_owned(),
        "ios:ABCDE12345.".to_owned(),
        "ios:ABCDE12345.app/x".to_owned(),
        "windows:anything".to_owned(),
    ] {
        let said = refusal(DEVICE_KEY_APPS, &format!("ios:ABCDE12345.ok,{wrong}"));
        assert!(
            said.as_deref()
                .is_some_and(|said| said.contains(&wrong) && !said.contains("ios:ABCDE12345.ok,")),
            "{wrong}: {said:?}"
        );
    }
    assert!(refusal(DEVICE_KEY_APPS, "ios:ABCDE12345.ok,")
        .is_some_and(|said| said.contains("an empty entry")));
    assert_eq!(refusal("gluetun:countries", "windows:anything"), None);
}
