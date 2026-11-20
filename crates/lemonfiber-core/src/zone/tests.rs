use super::Zone;
use crate::config::DEFAULT_ZONE;

#[test]
fn a_name_the_database_holds_is_a_zone() {
    for name in ["Europe/Amsterdam", "America/Chicago", "UTC"] {
        assert_eq!(
            Zone::read(name).map(|zone| zone.to_string()),
            Ok(name.to_owned())
        );
    }
}

#[test]
fn anything_else_is_refused_naming_the_setting() {
    for name in ["", "Mars/Olympus", "+02:00"] {
        assert_eq!(
            Zone::read(name),
            Err(format!(
                "TZ is the zone the stack runs in, a name from the time zone database such as \
                 Europe/Amsterdam, and {name} is not one"
            ))
        );
    }
}

#[test]
fn the_host_s_zone_is_proposed_and_the_default_where_it_names_none() {
    assert_eq!(Zone::proposed(Some("Asia/Tokyo")).as_str(), "Asia/Tokyo");
    for host in [None, Some(""), Some("Mars/Olympus")] {
        assert_eq!(Zone::proposed(host).as_str(), DEFAULT_ZONE);
    }
}

#[test]
fn it_is_read_and_written_as_its_name() {
    assert_eq!(
        serde_json::from_str::<Zone>("\"Asia/Tokyo\"")
            .ok()
            .and_then(|zone| serde_json::to_string(&zone).ok()),
        Some("\"Asia/Tokyo\"".to_owned())
    );
    assert!(serde_json::from_str::<Zone>("\"Mars/Olympus\"").is_err());
}

#[test]
fn the_host_s_zone_is_the_name_its_clock_keeps_and_none_where_it_keeps_none() {
    let amsterdam = jiff::tz::TimeZone::get(DEFAULT_ZONE).ok();
    assert_eq!(super::named(amsterdam), Some(DEFAULT_ZONE.to_owned()));
    assert_eq!(
        super::named(Some(jiff::tz::TimeZone::fixed(jiff::tz::offset(2)))),
        None
    );
    assert_eq!(super::named(None), None);
}
