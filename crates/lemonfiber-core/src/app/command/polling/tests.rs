use super::{Command, Polled, Surfacing};
use crate::app::command::{Queueing, Shelf, Whom};

/// Every read a stream polls, one of each.
pub(crate) fn every_polled() -> Vec<Polled> {
    vec![
        Polled::Settings,
        Polled::Wiring,
        Polled::Installed,
        Polled::Recorded,
        Polled::Copying,
        Polled::Playing { member: None },
        Polled::Playing {
            member: Some("a7f3".to_owned()),
        },
        Polled::Household { member: None },
        Polled::Household {
            member: Some(Whom::Named("a7f3".to_owned())),
        },
        Polled::Held(Shelf {
            member: Whom::Named("a7f3".to_owned()),
            most: 5,
        }),
    ]
}

#[test]
fn a_polled_read_is_asked_back_as_the_read_it_polls() {
    for polled in every_polled() {
        assert_eq!(
            polled.clone().twin().polled(),
            Command::Surface(Surfacing::Polled(polled))
        );
    }
}

#[test]
fn a_command_no_stream_polls_is_left_as_it_is() {
    for command in [
        Command::Surface(Surfacing::Standing),
        Command::Version,
        Command::Stack,
        Command::Queue(Queueing::Read),
    ] {
        assert_eq!(command.clone().polled(), command);
    }
}
