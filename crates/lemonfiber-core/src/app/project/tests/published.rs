//! The zone and the locale the core publishes to every service, as the house holds them.

use lemonfiber_manifest::container::{LOCALE, TIMEZONE};

use super::machine;
use crate::settings::Resolved;

#[test]
fn the_zone_and_the_locale_are_published_as_the_house_holds_them() {
    let (ctx, _) = machine("published");
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    let settings = env.with_file_name(crate::config::paths::SETTINGS);
    let read = |name: &str| Resolved::of(&ctx, None, &[]).setting(name);
    assert_eq!(read(TIMEZONE).as_deref(), Some(crate::config::DEFAULT_ZONE));
    assert_eq!(read(LOCALE).as_deref(), Some(crate::locale::DEFAULT));

    let chicago = "America/Chicago";
    assert!(crate::config::store::set(&env, crate::config::ZONE_KEY, chicago).is_ok());
    let _ = crate::settings::store::recorded(
        &settings,
        &[(crate::locale::SETTING, Some("nl-NL"))].into(),
    );
    assert_eq!(read(TIMEZONE).as_deref(), Some(chicago));
    assert_eq!(read(LOCALE).as_deref(), Some("nl-NL"));

    let _ = crate::settings::store::recorded(
        &settings,
        &[(crate::locale::SETTING, Some("nl_NL"))].into(),
    );
    assert_eq!(
        read(LOCALE).as_deref(),
        Some(crate::locale::DEFAULT),
        "a tag recorded in another shape is not published"
    );
    assert_eq!(
        Resolved::of(&ctx, None, &[])
            .changed(&[(crate::locale::SETTING, Some("de-DE"))].into())
            .setting(LOCALE)
            .as_deref(),
        Some("de-DE"),
        "a locale being changed is published as it is changed to"
    );
}
