use super::{Locale, DEFAULT};

#[test]
fn a_language_and_a_region_of_letters_or_digits_is_a_tag() {
    for tag in ["nl-NL", "en-US", "fil-PH", "es-419"] {
        assert_eq!(
            Locale::read(tag).map(|locale| locale.to_string()),
            Ok(tag.to_owned())
        );
    }
}

#[test]
fn anything_else_is_refused_naming_the_shape() {
    for text in [
        "",
        "nl",
        "nl_NL",
        "NL-nl",
        "nl-nl",
        "nl-NLD",
        "dutc-NL",
        "n-NL",
        "es-41",
        "nl-NL.UTF-8",
        "nl-NL-x",
    ] {
        assert_eq!(
            Locale::read(text),
            Err(format!(
                "LEMONFIBER_LOCALE is the house's language and region as one tag, \
                 two or three lowercase letters, a hyphen, then two uppercase letters or three \
                 digits, such as en-US, and {text} is not one"
            ))
        );
    }
}

#[test]
fn the_host_s_lang_proposes_its_language_and_territory() {
    assert_eq!(Locale::proposed(Some("nl_NL.UTF-8")).as_str(), "nl-NL");
    assert_eq!(Locale::proposed(Some("de_DE@euro")).as_str(), "de-DE");
    assert_eq!(Locale::proposed(Some("pt_BR")).as_str(), "pt-BR");
}

#[test]
fn a_host_that_names_no_such_tag_proposes_the_default() {
    for lang in [
        None,
        Some(""),
        Some("C"),
        Some("POSIX"),
        Some("C.UTF-8"),
        Some("en"),
    ] {
        assert_eq!(Locale::proposed(lang).as_str(), DEFAULT);
    }
}

#[test]
fn it_is_read_and_written_as_its_tag() {
    assert_eq!(
        serde_json::from_str::<Locale>("\"nl-NL\"")
            .ok()
            .and_then(|locale| serde_json::to_string(&locale).ok()),
        Some("\"nl-NL\"".to_owned())
    );
    assert!(serde_json::from_str::<Locale>("\"nl_NL\"").is_err());
    assert_eq!(
        serde_json::to_value(schemars::schema_for!(Locale))
            .ok()
            .and_then(|schema| schema.get("pattern").cloned()),
        Some(serde_json::json!("^[a-z]{2,3}-([A-Z]{2}|[0-9]{3})$"))
    );
}
