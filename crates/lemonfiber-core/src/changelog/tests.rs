use super::{notes, told, Record, Requirement, State, Summary, CARRIED, RECORD_DIR};

/// The newer of two releases, taken back.
const NEWER: &str = r##"{
  "version": "0.2.0", "tag": "v0.2.0", "released_on": "2026-02-01",
  "delivers": "The setup wizard", "patches": null, "carried": null,
  "withdrawn": "the installer shipped a broken pin", "user_facing": true,
  "groups": [{"title": "New", "entries": [
    {"summary": "Four removals", "requirements": ["A6-R1"], "reference": "#5"}
  ]}],
  "requirements": {"A6-R1": {"feature": "Clean uninstall", "url": "https://example.test/a6"}},
  "withdraws": {}, "withdrawn_requirements": []
}"##;

/// The older of the two, whose entries cite nothing.
const OLDER: &str = r#"{
  "version": "0.1.0", "tag": "v0.1.0", "released_on": "2026-01-01",
  "delivers": null, "patches": null, "carried": null, "withdrawn": null,
  "user_facing": false,
  "groups": [{"title": "Maintenance", "entries": [
    {"summary": "Bump a dependency", "requirements": []}
  ]}],
  "requirements": {},
  "withdraws": {}, "withdrawn_requirements": []
}"#;

fn two() -> Option<Record> {
    Record::read(&[("0.1.0.json", OLDER), ("0.2.0.json", NEWER)])
}

#[test]
fn files_that_are_not_a_record_are_read_as_none() {
    assert_eq!(Record::read(&[("0.1.0.json", "not json at all")]), None);
    assert_eq!(
        Record::read(&[("0.1.0.json", r#"{"version": "0.1.0"}"#)]),
        None
    );
    assert_eq!(Record::read(&[]), None);
}

#[test]
fn a_file_named_for_another_release_is_read_as_none() {
    assert_eq!(Record::read(&[("0.3.0.json", NEWER)]), None);
    assert_eq!(Record::read(&[("0.2.0", NEWER)]), None);
}

#[test]
fn a_release_whose_version_is_not_numbers_is_read_as_none() {
    let odd = NEWER.replace("\"0.2.0\"", "\"next\"");
    assert_eq!(Record::read(&[("next.json", odd.as_str())]), None);
}

#[test]
fn the_releases_are_read_newest_first_whatever_order_the_files_come_in() {
    let tenth = OLDER
        .replace("\"0.1.0\"", "\"0.10.0\"")
        .replace("v0.1.0", "v0.10.0");
    let read = Record::read(&[("0.10.0.json", tenth.as_str()), ("0.2.0.json", NEWER)]);
    assert_eq!(
        read.map(|record| record
            .releases
            .into_iter()
            .map(|one| one.version)
            .collect::<Vec<_>>()),
        Some(vec!["0.10.0".to_owned(), "0.2.0".to_owned()])
    );
}

#[test]
fn which_releases_shipped_a_requirement_is_read_off_their_entries() {
    let again = NEWER
        .replace("\"0.2.0\"", "\"0.3.0\"")
        .replace("v0.2.0", "v0.3.0");
    let read = Record::read(&[("0.2.0.json", NEWER), ("0.3.0.json", again.as_str())]);
    assert_eq!(
        read.and_then(|record| record.requirements.get("A6-R1").cloned()),
        Some(Requirement {
            feature: "Clean uninstall".to_owned(),
            url: Some("https://example.test/a6".to_owned()),
            withdrawn: false,
            shipped_in: vec!["0.3.0".to_owned(), "0.2.0".to_owned()],
        })
    );
}

/// A third release, saying the first release was taken back and a requirement the
/// second cites was withdrawn, both after their own files were written.
const LATER: &str = r#"{
  "version": "0.3.0", "tag": "v0.3.0", "released_on": null, "delivers": null,
  "patches": null, "carried": null, "withdrawn": null, "user_facing": false,
  "groups": [], "requirements": {},
  "withdraws": {"0.1.0": "it deleted the library"},
  "withdrawn_requirements": ["A6-R1"]
}"#;

#[test]
fn a_withdrawal_a_later_file_carries_is_folded_back_over_what_it_names() {
    let read = Record::read(&[
        ("0.1.0.json", OLDER),
        ("0.2.0.json", NEWER),
        ("0.3.0.json", LATER),
    ]);

    assert_eq!(
        read.as_ref()
            .and_then(|record| record.release("0.1.0"))
            .and_then(|one| one.withdrawn.clone()),
        Some("it deleted the library".to_owned())
    );
    // A release that was already withdrawn in its own file keeps its own reason.
    assert_eq!(
        read.as_ref()
            .and_then(|record| record.release("0.2.0"))
            .and_then(|one| one.withdrawn.clone()),
        Some("the installer shipped a broken pin".to_owned())
    );
    let gone = read.and_then(|record| record.requirements.get("A6-R1").cloned());
    assert_eq!(gone.as_ref().map(|one| one.withdrawn), Some(true));
    assert_eq!(gone.and_then(|one| one.url), None);
}

#[test]
fn the_record_this_build_carries_is_one_this_code_can_read() {
    // The artefact is generated and compiled in, so nothing at runtime would
    // report a shape this cannot parse — this is what would.
    let carried = Record::carried();
    assert!(carried.is_some(), "the carried record did not parse");
    assert_eq!(RECORD_DIR, "reference/changelog");
    assert_eq!(
        CARRIED
            .iter()
            .map(|(name, _)| *name)
            .find(|name| *name == "0.1.0.json"),
        Some("0.1.0.json")
    );
    // Destructured with a combinator rather than a `let ... else`: the arm for
    // a record that is certainly there is a line no test can ever run.
    assert_eq!(
        carried.as_ref().map(|record| record.releases.is_empty()),
        Some(false)
    );
    assert_eq!(
        carried
            .as_ref()
            .and_then(|record| record.release("0.1.0"))
            .map(|one| one.tag.clone()),
        Some("v0.1.0".to_owned())
    );
    assert_eq!(
        carried.and_then(|record| record.release("9.9.9").cloned()),
        None
    );
}

#[test]
fn the_record_this_build_carries_does_not_claim_a_release_after_it() {
    // The staleness rule, asked of the artefact this build actually ships: a
    // record naming something later than the workspace version is the file and
    // the build disagreeing about what went out. Which of the other two it is
    // depends on whether the workspace has been bumped past the last tag, and
    // both are honest — so what is asserted is the one that never is.
    assert_ne!(notes(env!("CARGO_PKG_VERSION")).state, State::Stale);
}

#[test]
fn a_build_whose_release_the_record_holds_is_current() {
    let said = told(two().as_ref(), "0.2.0");
    assert_eq!(said.state, State::Current);
    assert_eq!(
        said.running.map(|release| release.withdrawn),
        Some(Some("the installer shipped a broken pin".to_owned()))
    );
}

#[test]
fn a_build_cut_ahead_of_its_release_is_pending_rather_than_stale() {
    // `0.3.0-pre.1` precedes `0.3.0`. The record holds 0.1.0 and 0.2.0 and so
    // claims nothing this build could not have shipped — what is missing is notes
    // for a release that has not happened, which is the lag `Pending` names.
    // Read whole, the version is not a dotted run of numbers and every comparison
    // answers untellable, which would have called an honest record a contradiction.
    assert_eq!(told(two().as_ref(), "0.3.0-pre.1").state, State::Pending);
}

#[test]
fn a_version_that_is_not_one_is_still_a_contradiction() {
    // Dropping a suffix from something that was never a version leaves something
    // that still is not, so the answer this exists to give is unchanged.
    assert_eq!(told(two().as_ref(), "not-a-version").state, State::Stale);
    assert_eq!(told(two().as_ref(), "garbage").state, State::Stale);
}

#[test]
fn a_build_the_record_has_no_release_for_is_pending() {
    assert_eq!(told(two().as_ref(), "0.3.0").state, State::Pending);
}

#[test]
fn a_record_claiming_a_release_after_this_build_is_stale() {
    assert_eq!(told(two().as_ref(), "0.1.0").state, State::Stale);
}

#[test]
fn a_version_the_record_cannot_be_ordered_against_is_stale() {
    assert_eq!(told(two().as_ref(), "not-a-version").state, State::Stale);
}

#[test]
fn a_record_that_could_not_be_read_is_stale_and_empty() {
    let said = told(None, "0.2.0");
    assert_eq!(said.state, State::Stale);
    assert_eq!(said.running, None);
    assert_eq!(said.releases, Vec::new());
    assert!(said.requirements.is_empty());
}

#[test]
fn every_release_is_listed_whether_or_not_it_is_the_one_being_read() {
    let said = told(two().as_ref(), "0.2.0");
    assert_eq!(
        said.releases
            .iter()
            .map(|one| one.version.clone())
            .collect::<Vec<_>>(),
        vec!["0.2.0".to_owned(), "0.1.0".to_owned()]
    );
    // A release with nothing user-facing is in the listing saying so, rather
    // than being left out of it.
    assert_eq!(
        said.releases
            .iter()
            .map(|one| one.user_facing)
            .collect::<Vec<_>>(),
        vec![true, false]
    );
}

#[test]
fn only_the_requirements_the_release_being_read_cites_travel_with_it() {
    let said = told(two().as_ref(), "0.2.0");
    assert_eq!(
        said.requirements.keys().cloned().collect::<Vec<_>>(),
        vec!["A6-R1".to_owned()]
    );
    // The one the other release cites is in the record and not in this reading.
    assert_eq!(said.requirements.get("A6-R9"), None);
    // And a release whose entries cite nothing carries none of them.
    assert!(told(two().as_ref(), "0.1.0").requirements.is_empty());
}

#[test]
fn every_state_the_record_can_be_in_is_a_word_on_the_wire() {
    // The three are part of what a surface reads, so what each serialises to is
    // checked rather than left to the derive — a renamed variant would otherwise
    // change the contract without anything saying so.
    let words: Vec<String> = [State::Current, State::Pending, State::Stale]
        .iter()
        .map(|state| serde_json::to_string(state).unwrap_or_default())
        .collect();
    assert_eq!(
        words,
        vec![
            "\"current\"".to_owned(),
            "\"pending\"".to_owned(),
            "\"stale\"".to_owned()
        ]
    );
}

#[test]
fn a_listing_entry_keeps_what_a_reader_chooses_by_and_drops_the_rest() {
    // Asserted whole rather than field by field, for the reason above: what a
    // listing keeps and what it drops is one claim, and reading it out a field
    // at a time is three assertions that can each be true while the shape is
    // wrong.
    let summary = two()
        .as_ref()
        .and_then(|record| record.release("0.2.0"))
        .map(Summary::from);
    assert_eq!(
        summary,
        Some(Summary {
            version: "0.2.0".to_owned(),
            released_on: Some("2026-02-01".to_owned()),
            delivers: Some("The setup wizard".to_owned()),
            patches: None,
            withdrawn: Some("the installer shipped a broken pin".to_owned()),
            user_facing: true,
        })
    );
}
