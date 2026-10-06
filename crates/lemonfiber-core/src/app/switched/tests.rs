use super::off;
use crate::app::Command;
use crate::config::REACH_REGISTRY_KEY;

/// A context where `refused` is switched off, or nothing is.
fn reaching(refused: Option<&'static str>) -> crate::app::Ctx {
    crate::test_support::a_context()
        .settings(crate::config::Settings {
            reaching: refused.map_or_else(crate::config::Reaching::default, |one| {
                crate::config::Reaching::without(one)
            }),
            ..crate::config::Settings::default()
        })
        .build()
}

#[test]
fn a_fetch_is_unconfigured_while_fetching_images_is_switched_off() {
    let pull = Command::Pull {
        forms: vec!["library".to_owned()],
    };
    let off_here = reaching(Some(REACH_REGISTRY_KEY));
    assert_eq!(off(&off_here, &pull), Some(REACH_REGISTRY_KEY));
    let on_here = reaching(None);
    assert_eq!(off(&on_here, &pull), None);
}

#[test]
fn a_command_that_needs_no_setting_is_never_unconfigured() {
    let off_here = reaching(Some(REACH_REGISTRY_KEY));
    assert_eq!(off(&off_here, &Command::Seed), None);
}
