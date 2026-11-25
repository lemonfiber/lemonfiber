use std::time::Duration;

use lemonfiber_core::copying::{Copied, Copying, Running, State, What};
use lemonfiber_core::instant::Instant;
use tokio::time::Instant as Now;

use super::{Heard, COPYING_EVERY};

fn running(written: u64, part: &str, state: State) -> Copying {
    Copying {
        running: Some(Running {
            what: What::Backup,
            scope: Copied::WholeStack,
            job: None,
            started: Instant::from_seconds(1_795_563_600),
            written,
            whole: 100,
            part: part.to_owned(),
            state,
        }),
    }
}

#[test]
fn a_copy_is_said_as_it_starts_changes_part_or_state_and_ends() {
    let at = Now::now();
    let mut heard = Heard::default();
    assert!(
        heard.due(Copying::default(), false, at).is_some(),
        "nothing said at first"
    );
    assert!(
        heard.due(Copying::default(), false, at).is_none(),
        "none said twice"
    );
    assert!(
        heard
            .due(running(0, "a", State::Working), false, at)
            .is_some(),
        "start unsaid"
    );
    assert!(
        heard
            .due(running(5, "a", State::Working), false, at)
            .is_none(),
        "said for bytes alone"
    );
    assert!(
        heard
            .due(running(6, "b", State::Working), false, at)
            .is_some(),
        "part unsaid"
    );
    let stalled = State::Stalled {
        wrote: Instant::from_seconds(1_795_563_600),
    };
    assert!(
        heard.due(running(6, "b", stalled), false, at).is_some(),
        "stall unsaid"
    );
    assert!(
        heard.due(Copying::default(), false, at).is_some(),
        "end unsaid"
    );
}

#[test]
fn a_running_copy_is_said_every_five_seconds_and_to_whoever_joins() {
    let at = Now::now();
    let mut heard = Heard::default();
    let paced = State::Paced { limit: 9 };
    assert!(heard.due(running(1, "a", paced), false, at).is_some());
    assert!(heard
        .due(running(2, "a", paced), false, at + Duration::from_secs(4))
        .is_none());
    assert!(heard
        .due(running(3, "a", paced), true, at + Duration::from_secs(4))
        .is_some());
    let later = at + Duration::from_secs(4) + COPYING_EVERY;
    assert!(heard.due(running(4, "a", paced), false, later).is_some());
}

#[test]
fn nothing_running_is_said_only_to_whoever_joins() {
    let at = Now::now();
    let mut heard = Heard::default();
    assert!(heard.due(Copying::default(), false, at).is_some());
    assert!(heard
        .due(Copying::default(), false, at + 10 * COPYING_EVERY)
        .is_none());
    assert!(heard
        .due(Copying::default(), true, at + 10 * COPYING_EVERY)
        .is_some());
}
