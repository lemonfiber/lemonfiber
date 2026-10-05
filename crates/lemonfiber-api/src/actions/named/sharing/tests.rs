use super::{about_the_line, asked_for};
use crate::actions::Arguments;
use lemonfiber_core::app::Command;

#[test]
fn only_the_lines_own_actions_are_about_the_line() {
    for action in ["bandwidth", "downloads-pause", "downloads-resume"] {
        assert!(about_the_line(action), "{action}");
    }
    assert!(!about_the_line("space"));
    assert!(!about_the_line("household"));
}

#[test]
fn a_pause_and_a_resume_name_every_download_client_at_once() {
    use lemonfiber_core::bandwidth::Pausing;
    assert_eq!(
        asked_for("downloads-pause", Arguments::default()),
        Command::Downloads(Pausing::Pause)
    );
    assert_eq!(
        asked_for("downloads-resume", Arguments::default()),
        Command::Downloads(Pausing::Resume)
    );
}

#[test]
fn nothing_given_is_the_reading_every_surface_makes_first() {
    assert_eq!(
        asked_for("bandwidth", Arguments::default()),
        Command::Bandwidth(lemonfiber_core::app::BandwidthAsked::default())
    );
}

#[test]
fn every_word_reaches_the_command_as_it_was_written() {
    let given = Arguments {
        down: Some("50%".to_owned()),
        up: Some("2MiB".to_owned()),
        active: Some("07:00-23:00".to_owned()),
        line: Some("60MiB/6MiB".to_owned()),
        cap: Some("1TiB".to_owned()),
        exceeded: Some("pause".to_owned()),
        unrestricted_for: Some(90),
        ..Arguments::default()
    };
    assert_eq!(
        asked_for("bandwidth", given),
        Command::Bandwidth(lemonfiber_core::app::BandwidthAsked {
            down: Some("50%".to_owned()),
            up: Some("2MiB".to_owned()),
            active: Some("07:00-23:00".to_owned()),
            line: Some("60MiB/6MiB".to_owned()),
            cap: Some("1TiB".to_owned()),
            exceeded: Some("pause".to_owned()),
            unrestricted_for: Some(90),
        })
    );
}
