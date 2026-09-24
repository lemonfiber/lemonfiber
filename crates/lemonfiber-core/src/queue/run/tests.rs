use super::{watch, Answered, Watched};
use crate::condition::Conditions;
use crate::ports::service::Queued;
use crate::queue::{Stall, Stuck, Thresholds};

/// One thing an \*arr is waiting for, fetched once.
fn queued(title: &str, status: &str, message: Option<&str>) -> Queued {
    grabbed(title, status, message, 1)
}

/// The same, fetched however many times since it last imported.
fn grabbed(title: &str, status: &str, message: Option<&str>, grabs: u32) -> Queued {
    Queued {
        title: title.to_owned(),
        status: status.to_owned(),
        state: "downloading".to_owned(),
        message: message.map(str::to_owned),
        download_id: None,
        grabs,
    }
}

/// One service answering with these items.
fn sonarr(items: Vec<Queued>) -> Vec<(String, Answered)> {
    vec![("sonarr".to_owned(), Answered::Queue(items))]
}

/// What one pass over the pipeline made of it.
fn watched(
    answers: &[(String, Answered)],
    fetching: &[(String, u8, bool)],
    conditions: &mut Conditions,
    now: &str,
) -> Watched {
    watch(
        answers,
        fetching,
        conditions,
        Thresholds::conservative(),
        now,
    )
}

#[test]
fn a_fault_says_nothing_until_it_has_held_long_enough() {
    // The age is the store's, not the item's: the first pass records it and
    // says nothing, and only a later one reports it.
    let mut conditions = Conditions::new();
    let answers = sonarr(vec![queued("Some.Release", "ok", None)]);
    let fetching = vec![("Some.Release".to_owned(), 42, false)];

    let first = watched(&answers, &fetching, &mut conditions, "1000");
    assert!(first.stuck.is_empty(), "seen once, said nothing");

    let later = watched(&answers, &fetching, &mut conditions, "100000");
    let reported: Vec<Stall> = later.stuck.iter().map(|stuck| stuck.stall).collect();
    assert_eq!(reported, vec![Stall::StalledDownload]);
}

#[test]
fn a_stall_that_resolved_itself_is_cleared_rather_than_left_standing() {
    // The item recovers, so it is simply not raised on the next pass — and the
    // store records the resolution, which is what an operator reading the
    // history needs to see.
    let mut conditions = Conditions::new();
    let answers = sonarr(vec![queued("Some.Release", "ok", None)]);

    watched(
        &answers,
        &[("Some.Release".to_owned(), 42, false)],
        &mut conditions,
        "1000",
    );
    let moving = watched(
        &answers,
        &[("Some.Release".to_owned(), 60, true)],
        &mut conditions,
        "100000",
    );

    assert!(
        moving.stuck.is_empty(),
        "it is moving again: {:?}",
        moving.stuck
    );
    let stalled_cleared = conditions
        .get("queue.stalled.Some.Release")
        .map(crate::condition::Condition::is_raised);
    assert_eq!(
        stalled_cleared,
        Some(false),
        "the stall is recorded as over"
    );
}

#[test]
fn a_torrent_seeding_after_a_successful_import_is_never_called_an_orphan() {
    // From here the two are identical: the queue holds what is in progress, so
    // an imported item has left it — and a finished download nobody is waiting
    // for is either an orphan or a healthy seed. Guessing orphan would flag
    // every seeding torrent on the machine, which is the false positive the
    // whole feature exists to avoid.
    let mut conditions = Conditions::new();
    let complete = [("Some.Release".to_owned(), 100u8, false)];
    watched(&sonarr(Vec::new()), &complete, &mut conditions, "1000");
    let later = watched(&sonarr(Vec::new()), &complete, &mut conditions, "9000000");
    assert!(later.stuck.is_empty(), "{:?}", later.stuck);
}

#[test]
fn a_service_that_could_not_be_asked_is_named_rather_than_read_as_empty() {
    // Silence is not an empty queue. Reporting it as one is how an operator
    // comes to believe a pipeline is idle when it is unreachable.
    let mut conditions = Conditions::new();
    let answers = vec![("sonarr".to_owned(), Answered::Unreachable)];
    let watched = watched(&answers, &[], &mut conditions, "1000");
    assert_eq!(watched.unverified, vec!["sonarr".to_owned()]);
    assert!(watched.stuck.is_empty());
}

#[test]
fn a_long_queue_is_summarised_by_category_rather_than_listed() {
    // Twenty stalled downloads are one sentence and one remedy. Printing twenty
    // lines is how a report stops being read where it starts mattering.
    let mut conditions = Conditions::new();
    let items: Vec<Queued> = (0..20)
        .map(|n| queued(&format!("Release.{n}"), "ok", None))
        .collect();
    let fetching: Vec<(String, u8, bool)> = (0..20)
        .map(|n| (format!("Release.{n}"), 42, false))
        .collect();
    watched(&sonarr(items.clone()), &fetching, &mut conditions, "1000");
    let later = watched(&sonarr(items), &fetching, &mut conditions, "100000");

    assert_eq!(later.stuck.len(), 20);
    assert_eq!(later.by_category(), vec![(Stall::StalledDownload, 20)]);
}

#[test]
fn an_import_the_service_complains_about_once_is_not_yet_called_repeated() {
    // The service says it is failing, never how many times. Claiming repetition
    // from a single complaint would be claiming to have watched something
    // nobody watched — it is a finished download nothing has taken, which is
    // what it is, and the cause is named beside it.
    let mut conditions = Conditions::new();
    let answers = sonarr(vec![queued(
        "Some.Release",
        "warning",
        Some("Permission denied writing to /data/media"),
    )]);
    let fetching = vec![("Some.Release".to_owned(), 100, false)];
    watched(&answers, &fetching, &mut conditions, "1000");
    let later = watched(&answers, &fetching, &mut conditions, "100000");
    let reported: Vec<Stall> = later.stuck.iter().map(|stuck| stuck.stall).collect();
    assert_eq!(reported, vec![Stall::CompletedNotImported]);
}

#[test]
fn an_import_that_has_failed_come_back_and_failed_again_is_structural() {
    // Repetition the store actually observed: the fault cleared and returned.
    // That is a different problem from one bad import, and it will not resolve
    // itself.
    let mut conditions = Conditions::new();
    let complaining = sonarr(vec![queued("Some.Release", "warning", Some("denied"))]);
    let fine = sonarr(Vec::new());
    let stuck_at_100 = vec![("Some.Release".to_owned(), 100, false)];
    let imported: Vec<(String, u8, bool)> = Vec::new();

    // One cycle more than the threshold: the count is read before this pass
    // raises anything, so it is how many times it had come back *before* now.
    for pass in 0..=crate::queue::REPEATED {
        let at = format!("{}", 1000 + u64::from(pass) * 10);
        watched(&complaining, &stuck_at_100, &mut conditions, &at);
        // It leaves the pipeline entirely — imported, gone from both sides —
        // and then comes back, which is what the store counts.
        watched(&fine, &imported, &mut conditions, &at);
    }
    let again = watched(&complaining, &stuck_at_100, &mut conditions, "100000");
    let reported: Vec<Stall> = again.stuck.iter().map(|stuck| stuck.stall).collect();
    assert_eq!(reported, vec![Stall::RepeatedImportFailure]);
}

#[test]
fn a_stall_nothing_explained_says_what_is_usually_behind_it() {
    // The stall with no stated cause is the one an operator is least equipped to
    // start on, so it is exactly the one that says where to look. Where the
    // service did say why, that is carried instead — what actually happened beats
    // what usually does.
    let mut conditions = Conditions::new();
    let answers = sonarr(vec![queued("Some.Release", "ok", None)]);
    watched(
        &answers,
        &[("Some.Release".to_owned(), 42, false)],
        &mut conditions,
        "1000",
    );
    let said = conditions
        .get("queue.stalled.Some.Release")
        .map(|condition| condition.summary.clone())
        .unwrap_or_default();
    assert!(said.contains("Some.Release"), "{said}");
    assert!(said.contains("typically"), "{said}");
}

#[test]
fn the_condition_a_stuck_item_raises_carries_its_remedies() {
    // What an operator does about it, in the category's own words — a stuck
    // item they can do nothing with is a status line.
    let mut conditions = Conditions::new();
    let answers = sonarr(vec![queued("Some.Release", "ok", None)]);
    watched(
        &answers,
        &[("Some.Release".to_owned(), 42, false)],
        &mut conditions,
        "1000",
    );
    let remedies = conditions
        .get("queue.stalled.Some.Release")
        .map(|condition| condition.remedies.clone())
        .unwrap_or_default();
    assert!(remedies.len() >= 2, "{remedies:?}");
}

#[test]
fn a_download_that_started_moving_again_stops_being_reported() {
    // The other way a stall resolves: still in the pipeline, but going again.
    let mut conditions = Conditions::new();
    let answers = sonarr(vec![queued("Some.Release", "ok", None)]);
    let stopped = [("Some.Release".to_owned(), 42u8, false)];
    watched(&answers, &stopped, &mut conditions, "1000");
    let reported = watched(&answers, &stopped, &mut conditions, "100000");
    assert_eq!(
        reported
            .stuck
            .iter()
            .map(|stuck| stuck.stall)
            .collect::<Vec<_>>(),
        vec![Stall::StalledDownload]
    );

    let moving = [("Some.Release".to_owned(), 60u8, true)];
    let going = watched(&answers, &moving, &mut conditions, "100010");
    assert!(
        going.stuck.is_empty(),
        "the stall is gone and the slow note has not yet held: {:?}",
        going.stuck
    );
}

#[test]
fn something_monitored_that_nothing_has_fetched_is_waiting() {
    // Only the *arr knows about it: nothing is downloading, so there is no
    // stall to report — it is waiting, and the longest rope of all applies.
    let mut conditions = Conditions::new();
    let answers = sonarr(vec![queued("Some.Film", "ok", None)]);
    watched(&answers, &[], &mut conditions, "1000");
    let much_later = watched(&answers, &[], &mut conditions, "9000000");
    let reported: Vec<Stall> = much_later.stuck.iter().map(|stuck| stuck.stall).collect();
    assert_eq!(reported, vec![Stall::WaitingIndefinitely]);
}

#[test]
fn every_category_files_under_a_kind_of_its_own() {
    // The kind is what groups four items stuck the same way into one alert, so
    // two categories sharing one would silently merge two different problems.
    let mut kinds: Vec<&str> = Stall::ALL.into_iter().map(super::kind_of).collect();
    let count = kinds.len();
    kinds.sort_unstable();
    kinds.dedup();
    assert_eq!(kinds.len(), count, "two categories share a kind");
    assert!(kinds.iter().all(|kind| !kind.is_empty()));
}

#[test]
fn one_cause_stopping_several_downloads_is_reported_once() {
    // A full disk stops everything on the machine. Twenty conditions about it
    // are twenty alerts for one thing to fix, which is how an operator learns
    // to mute the check that would have told them.
    let full = Some("No space left on device");
    let answers = sonarr(vec![
        queued("First.Release", "warning", full),
        queued("Second.Release", "warning", full),
        queued("Third.Release", "warning", full),
    ]);
    // Each is downloading and stopped, which is what a full disk does to a
    // download: the client still holds it and nothing is moving.
    let stopped = [
        ("First.Release".to_owned(), 40u8, false),
        ("Second.Release".to_owned(), 40u8, false),
        ("Third.Release".to_owned(), 40u8, false),
    ];
    let mut conditions = Conditions::new();
    watched(&answers, &stopped, &mut conditions, "1000");
    let reported = watched(&answers, &stopped, &mut conditions, "100000");

    let said: Vec<String> = reported.stuck.iter().map(Stuck::said).collect();
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said.first().is_some_and(
            |line| line.contains("3 items") && line.contains("No space left on device")
        ),
        "{said:?}"
    );
    let raised: Vec<String> = conditions
        .raised()
        .iter()
        .map(|condition| condition.check.clone())
        .collect();
    assert_eq!(raised, vec!["queue.blocked.No space left on device"]);
}

#[test]
fn one_item_blocked_by_something_is_still_named_by_its_own_name() {
    // A single item blocked by something is that item's problem, and naming the
    // cause instead would lose which download to look at.
    let answers = sonarr(vec![queued(
        "Only.Release",
        "warning",
        Some("Permission denied"),
    )]);
    let stopped = [("Only.Release".to_owned(), 40u8, false)];
    let mut conditions = Conditions::new();
    watched(&answers, &stopped, &mut conditions, "1000");
    let reported = watched(&answers, &stopped, &mut conditions, "100000");
    let said: Vec<String> = reported.stuck.iter().map(Stuck::said).collect();
    assert!(
        said.first().is_some_and(
            |line| line.starts_with("Only.Release") && line.ends_with(": Permission denied")
        ),
        "{said:?}"
    );
}

#[test]
fn two_items_stopped_by_different_things_stay_two() {
    // Grouping is about one cause, not about tidiness: two different faults are
    // two things to fix, and folding them together would hide one of them.
    let answers = sonarr(vec![
        queued("First.Release", "warning", Some("No space left on device")),
        queued("Second.Release", "warning", Some("Permission denied")),
    ]);
    let stopped = [
        ("First.Release".to_owned(), 40u8, false),
        ("Second.Release".to_owned(), 40u8, false),
    ];
    let mut conditions = Conditions::new();
    watched(&answers, &stopped, &mut conditions, "1000");
    let reported = watched(&answers, &stopped, &mut conditions, "100000");
    assert_eq!(reported.stuck.len(), 2);
}

#[test]
fn an_item_fetched_over_and_over_is_reported_as_the_loop_it_is() {
    // The category the model has always carried and nothing could reach: the
    // count comes from the service's own history, and without it a loop reads
    // as an ordinary download.
    let answers = sonarr(vec![grabbed("Some.Release", "ok", None, 3)]);
    let moving = [("Some.Release".to_owned(), 40u8, true)];
    let mut conditions = Conditions::new();
    watched(&answers, &moving, &mut conditions, "1000");
    let reported = watched(&answers, &moving, &mut conditions, "100000");
    assert_eq!(
        reported
            .stuck
            .iter()
            .map(|stuck| stuck.stall)
            .collect::<Vec<_>>(),
        vec![Stall::RedownloadLoop]
    );
}

#[test]
fn an_item_fetched_once_is_not_a_loop() {
    // Twice is a retry, which is a system working. The count has to come from
    // somewhere real, or every download reads as a loop.
    let answers = sonarr(vec![grabbed("Some.Release", "ok", None, 1)]);
    let moving = [("Some.Release".to_owned(), 40u8, true)];
    let mut conditions = Conditions::new();
    watched(&answers, &moving, &mut conditions, "1000");
    let reported = watched(&answers, &moving, &mut conditions, "100000");
    assert!(reported
        .stuck
        .iter()
        .all(|stuck| stuck.stall != Stall::RedownloadLoop));
}
