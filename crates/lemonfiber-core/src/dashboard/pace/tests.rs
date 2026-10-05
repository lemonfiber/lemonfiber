use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{Paced, Readings, VPN_EVERY};

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

/// A panel never read is due, and one read is not due again until its pace has passed.
#[test]
fn a_panel_is_due_when_never_read_and_once_its_pace_has_passed() {
    let mut readings = Readings::default();
    assert!(readings.due(Paced::Vpn, at(1_000)), "never read");

    readings.read(Paced::Vpn, at(1_000));
    assert!(!readings.due(Paced::Vpn, at(1_001)), "read a second ago");
    assert!(
        !readings.due(Paced::Vpn, at(1_000) + VPN_EVERY - Duration::from_secs(1)),
        "not yet"
    );
    assert!(
        readings.due(Paced::Vpn, at(1_000) + VPN_EVERY),
        "its pace has passed"
    );
    assert!(
        readings.due(Paced::Household, at(1_001)),
        "each panel keeps its own pace"
    );
}

/// A clock set back is no evidence the last reading is current.
#[test]
fn a_clock_set_back_makes_a_panel_due() {
    let mut readings = Readings::default();
    readings.read(Paced::Queues, at(1_000));
    assert!(readings.due(Paced::Queues, at(999)));
}

/// Every panel is read less often than the screen refreshes, and none is let go for
/// longer than the slowest of them.
#[test]
fn every_paced_panel_is_slower_than_the_screen_and_none_slower_than_the_probe() {
    for panel in Paced::ALL {
        assert!(panel.every() > crate::dashboard::TICK, "{panel:?}");
        assert!(panel.every() <= super::HARDLINK_EVERY, "{panel:?}");
    }
}
