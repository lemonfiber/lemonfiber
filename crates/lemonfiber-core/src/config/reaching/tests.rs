use super::{
    env::EnvFile, offline, Reaching, OFFLINE_KEY, REACH_GUIDES_KEY, REACH_USENET_KEY, SWITCHES,
};

#[test]
fn a_file_that_says_nothing_leaves_every_request_allowed() {
    let allowed = Reaching::from_env(&EnvFile::parse("PUID=1000\n"));
    assert_eq!(allowed, Reaching::default());
    for switch in SWITCHES {
        assert!(allowed.allows(switch), "{switch} was refused by nothing");
    }
}

#[test]
fn each_request_is_switched_off_on_its_own() {
    for switch in SWITCHES {
        let allowed = Reaching::from_env(&EnvFile::parse(&format!("{switch}=off\n")));
        assert!(
            !allowed.allows(switch),
            "{switch} did not switch its own off"
        );
        let others = SWITCHES
            .iter()
            .filter(|other| *other != switch && allowed.allows(other))
            .count();
        assert_eq!(
            others,
            SWITCHES.len() - 1,
            "{switch} switched off something else as well"
        );
        assert_eq!(allowed, Reaching::without(switch));
    }
}

#[test]
fn the_blanket_switch_stops_all_of_them() {
    let file = EnvFile::parse(&format!("{OFFLINE_KEY}=on\n"));
    assert!(offline(&file));
    assert_eq!(Reaching::from_env(&file), Reaching::none());
    for switch in SWITCHES {
        assert!(!Reaching::none().allows(switch), "{switch}");
    }
}

/// A file edited in two sittings says both things, and the narrower reading is
/// the safe one — nothing leaves, which is what the blanket switch was set for.
#[test]
fn the_blanket_switch_wins_over_a_request_left_switched_on() {
    let file = EnvFile::parse(&format!("{OFFLINE_KEY}=on\n{REACH_GUIDES_KEY}=on\n"));
    assert_eq!(Reaching::from_env(&file), Reaching::none());
}

#[test]
fn a_blanket_switch_that_is_off_leaves_the_individual_answers_alone() {
    let file = EnvFile::parse(&format!("{OFFLINE_KEY}=off\n{REACH_USENET_KEY}=off\n"));
    assert!(!offline(&file));
    assert_eq!(
        Reaching::from_env(&file),
        Reaching::without(REACH_USENET_KEY)
    );
}

/// A caller asking about a setting nobody has heard of is asking whether the
/// operator switched it off, and they did not.
#[test]
fn a_setting_this_product_does_not_read_is_not_something_anybody_switched_off() {
    assert!(Reaching::none().allows("LEMONFIBER_REACH_SOMETHING_ELSE"));
}
