use crate::settings::own::{
    is_configs, is_core, refusal, DASHBOARD_SEARCH, DEFAULT_SEARCH, DEVICE_KEY_APPS, DOMAIN,
};

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

#[test]
fn the_dashboards_search_address_is_a_core_setting() {
    assert!(is_core(DASHBOARD_SEARCH));
    assert!(!is_configs(DASHBOARD_SEARCH));
}

#[test]
fn an_absolute_web_address_is_taken_for_the_dashboards_search() {
    for value in [
        DEFAULT_SEARCH,
        "https://www.ecosia.org/search?q=",
        "http://search.home.arpa/?q=",
    ] {
        assert_eq!(refusal(DASHBOARD_SEARCH, value), None, "{value}");
    }
}

#[test]
fn anything_but_an_absolute_web_address_is_refused_naming_the_setting() {
    for value in [
        "",
        "duckduckgo.com/?q=",
        "/search?q=",
        "ftp://example.com/?q=",
        "javascript:alert(1)",
        "https:///?q=",
        "https://example.com/?q= more",
    ] {
        assert!(
            refusal(DASHBOARD_SEARCH, value).is_some_and(|said| said.starts_with(DASHBOARD_SEARCH)),
            "{value:?}"
        );
    }
}

#[test]
fn the_dashboards_search_address_is_the_one_recorded_or_the_default_and_a_bad_one_is_refused() {
    let scratch = lemonfiber_fixtures::scratch::Scratch::new("dashboard-search");
    let env = scratch.path().join(".env");
    let _ = std::fs::write(&env, "");
    let ctx = crate::test_support::a_context()
        .settings(crate::config::Settings {
            env_file: Some(env.clone()),
            ..crate::config::Settings::default()
        })
        .build();
    let settings = env.with_file_name(crate::config::paths::SETTINGS);
    let record = |value: &str| {
        crate::settings::store::recorded(&settings, &[(DASHBOARD_SEARCH, Some(value))].into())
            .is_ok()
    };

    assert_eq!(
        crate::settings::dashboard_search(&ctx),
        Ok(DEFAULT_SEARCH.to_owned())
    );
    assert!(record("https://www.ecosia.org/search?q="));
    assert_eq!(
        crate::settings::dashboard_search(&ctx),
        Ok("https://www.ecosia.org/search?q=".to_owned())
    );
    assert!(record("ecosia.org/search?q="));
    assert!(crate::settings::dashboard_search(&ctx)
        .is_err_and(|said| said.starts_with(DASHBOARD_SEARCH)));
}

#[test]
fn a_domain_is_taken_and_anything_a_name_cannot_be_put_in_front_of_is_refused() {
    assert!(is_core(DOMAIN));
    for value in ["home.example", "nas", "my-home.example.org"] {
        assert_eq!(refusal(DOMAIN, value), None, "{value}");
    }
    let long = "a".repeat(64);
    let longest = ["a"; 128].join(".");
    for value in [
        "",
        "home..example",
        "-home.example",
        "home-.example",
        "home example",
        "https://home.example",
        long.as_str(),
        longest.as_str(),
    ] {
        assert!(
            refusal(DOMAIN, value).is_some_and(|said| said.contains(DOMAIN)),
            "{value}"
        );
    }
}
