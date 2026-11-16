use super::{Cron, WallClock};

/// Monday 3 March at `hour`:`minute`.
const fn monday(hour: u8, minute: u8) -> WallClock {
    WallClock {
        minute,
        hour,
        day: 3,
        month: 3,
        weekday: 1,
    }
}

fn fires(text: &str, at: WallClock) -> bool {
    Cron::parse(text).is_some_and(|cron| cron.fires(at))
}

#[test]
fn a_five_field_expression_fires_in_the_minutes_it_names() {
    assert!(fires("30 4 * * *", monday(4, 30)));
    assert!(!fires("30 4 * * *", monday(4, 31)));
    assert!(fires("*/15 9-17 * * 1-5", monday(9, 45)));
    assert!(!fires("*/15 9-17 * * 1-5", monday(18, 0)));
    assert!(fires("5,35 * * 3 *", monday(12, 35)));
    assert!(fires("10/20 * * * *", monday(0, 50)));
    assert!(!fires("10/20 * * * *", monday(0, 0)));
}

#[test]
fn each_shortcut_fires_as_the_expression_it_stands_for() {
    for (shortcut, expression) in super::SHORTCUTS {
        assert_eq!(Cron::parse(shortcut), Cron::parse(expression), "{shortcut}");
    }
    assert!(fires("@hourly", monday(7, 0)));
    assert!(fires("@daily", monday(0, 0)));
    assert!(!fires("@weekly", monday(0, 0)));
    assert!(!fires("@monthly", monday(0, 0)));
}

#[test]
fn sunday_is_both_zero_and_seven() {
    let sunday = WallClock {
        weekday: 0,
        ..monday(0, 0)
    };
    assert!(fires("0 0 * * 7", sunday));
    assert_eq!(Cron::parse("0 0 * * 7"), Cron::parse("0 0 * * 0"));
}

/// Narrowed both ways, either the day of the month or the day of the week is enough.
#[test]
fn a_day_of_the_month_and_a_day_of_the_week_both_narrowed_fire_on_either() {
    assert!(fires("0 0 1 * 1", monday(0, 0)));
    assert!(fires("0 0 3 * 5", monday(0, 0)));
    assert!(!fires("0 0 1 * 5", monday(0, 0)));
    assert!(!fires("0 0 1 * *", monday(0, 0)));
}

#[test]
fn anything_but_five_fields_in_bounds_or_a_shortcut_is_no_schedule() {
    for text in [
        "",
        "@yearly",
        "@reboot",
        "* * * *",
        "* * * * * *",
        "60 * * * *",
        "* 24 * * *",
        "* * 0 * *",
        "* * * 13 *",
        "* * * * 8",
        "*/0 * * * *",
        "5-1 * * * *",
        "1,,2 * * * *",
        "MON * * * *",
        "-1 * * * *",
    ] {
        assert_eq!(Cron::parse(text), None, "{text:?}");
    }
}
