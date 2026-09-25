use super::{notes, told, Record, State, Summary, CARRIED, RECORD_PATH};

/// A record of two releases, one of them taken back.
const TWO: &str = r##"{
  "releases": [
    {
      "version": "0.2.0", "tag": "v0.2.0", "released_on": "2026-02-01",
      "delivers": "The setup wizard", "patches": null, "carried": null,
      "withdrawn": "the installer shipped a broken pin", "user_facing": true,
      "groups": [{"title": "New", "entries": [
        {"summary": "Four removals", "requirements": ["A6-R1"], "reference": "#5"}
      ]}]
    },
    {
      "version": "0.1.0", "tag": "v0.1.0", "released_on": "2026-01-01",
      "delivers": null, "patches": null, "carried": null, "withdrawn": null,
      "user_facing": false,
      "groups": [{"title": "Maintenance", "entries": [
        {"summary": "Bump a dependency", "requirements": []}
      ]}]
    }
  ],
  "requirements": {
    "A6-R1": {"feature": "Clean uninstall", "url": "https://example.test/a6",
              "shipped_in": ["0.2.0"]},
    "A6-R9": {"feature": "Clean uninstall", "withdrawn": true,
              "shipped_in": ["0.1.0"]}
  }
}"##;

fn two() -> Option<Record> {
    Record::read(TWO)
}

#[test]
fn a_document_that_is_not_a_record_is_read_as_none() {
    assert_eq!(Record::read("not json at all"), None);
    assert_eq!(Record::read(r#"{"releases": []}"#), None);
}

#[test]
fn the_record_this_build_carries_is_one_this_code_can_read() {
    // The artefact is generated and compiled in, so nothing at runtime would
    // report a shape this cannot parse — this is what would.
    let carried = Record::carried();
    assert!(carried.is_some(), "the carried record did not parse");
    assert!(Record::read(CARRIED).is_some());
    assert_eq!(RECORD_PATH, "reference/changelog.json");
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
