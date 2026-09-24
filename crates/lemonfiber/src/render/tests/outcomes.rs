//! Every outcome renders, and the setup walk says where it is.

use super::*;

/// A setup part-way through, which is what most of these vary from.
fn part_way() -> WizardReport {
    WizardReport {
        offered: true,
        phase: Phase::InProgress,
        at: Step::DataLocation,
        asks: true,
        unanswered: vec![Step::DataLocation, Step::Library],
        ready_for_review: false,
        plan: Vec::new(),
        written: Vec::new(),
        proof: None,
    }
}

/// Every state a removal can end in renders, which the one above cannot show:
/// it is `partial`, and the other three carry different paragraphs.
#[test]
fn every_state_a_removal_ends_in_renders() {
    use lemonfiber_core::uninstall::{Removal, Uninstall};

    let one = a_removal();
    for removal in [
        Removal::Surveyed,
        Removal::Confirmed,
        Removal::Complete {
            gone: Vec::new(),
            credentials: Vec::new(),
        },
    ] {
        let rendered = answer(
            &Outcome::Uninstall(Uninstall {
                manifest: one.manifest.clone(),
                removal,
            }),
            false,
        )
        .text();
        assert!(!rendered.is_empty(), "{rendered}");
    }
}

/// A removal where every line goes says how many go and nothing about what is
/// kept, because there is nothing kept to say anything about.
#[test]
fn a_removal_that_keeps_none_of_what_it_lists_counts_only_what_goes() {
    use lemonfiber_core::uninstall::{Item, Manifest, Uninstall};

    let one = a_removal();
    let whole = Uninstall {
        manifest: Manifest {
            items: one
                .manifest
                .items
                .iter()
                .map(|item| Item {
                    kept: None,
                    ..item.clone()
                })
                .collect(),
            ..one.manifest.clone()
        },
        removal: one.removal.clone(),
    };

    let rendered = answer(&Outcome::Uninstall(whole), false).text();
    assert!(rendered.contains("2 to remove —"), "{rendered}");
    assert!(!rendered.contains("left where they are"), "{rendered}");
}

/// A reading that found nothing still says so, rather than printing a heading
/// over an empty list.
#[test]
fn a_removal_that_found_nothing_says_so() {
    use lemonfiber_core::uninstall::{Confidence, Manifest, Removal, Tier, Uninstall};

    let nothing = Uninstall {
        manifest: Manifest {
            tier: Tier::Stop,
            removes: Tier::Stop.removes().to_owned(),
            keeps: Tier::Stop.keeps().to_owned(),
            items: Vec::new(),
            bytes: 0,
            foreign: Vec::new(),
            volume: None,
            coming: Vec::new(),
            outside: Vec::new(),
            backup: None,
            confidence: Confidence::whole(),
            agreement: "deadbeef".to_owned(),
        },
        removal: Removal::Surveyed,
    };

    let rendered = answer(&Outcome::Uninstall(nothing), false).text();
    assert!(rendered.contains("Nothing of this was found"), "{rendered}");
}

#[test]
fn every_outcome_renders_and_every_outcome_renders_as_json() {
    // Every arm of the dispatch renders something, and every one of them also
    // renders as an envelope a script can parse.
    let outcomes = every_outcome();
    // *Every* outcome, so the list has to be one: a fixture that came back empty
    // would satisfy both claims below without rendering anything.
    assert!(!outcomes.is_empty(), "no outcome was gathered to render");
    for outcome in outcomes {
        assert!(!answer(&outcome, false).text().is_empty());
        let json = answer(&outcome, true).text();
        assert!(json.contains(r#""api_version""#), "{json}");
    }
}

#[test]
fn the_json_fallback_is_a_value_rather_than_an_unreachable_branch() {
    // Nothing can actually fail to serialise; the fallback exists so the line is
    // one a test can reach, and this is that test.
    assert!(machine_readable(&Outcome::Version(a_version()))
        .text()
        .contains(r#""kind":"version""#));
    assert!(!super::super::UNRENDERABLE.is_empty());
}

#[test]
fn a_name_from_somewhere_else_cannot_take_over_the_terminal() {
    // Most of what is shown here came from an indexer or a service, and a
    // terminal reads a control character in the middle of one as an
    // instruction. Caught at the one place every line goes through, so a
    // renderer added later cannot forget it.
    let mut lines = Lines::default();
    lines.put("Some\u{1b}[2JRelease");
    lines.spaced("and\rthis");
    lines.block("a\u{7f}b");
    assert_eq!(lines.text(), "Some[2JRelease\n\nandthis\nab");
}

#[test]
fn a_setup_part_way_through_names_where_it_is_and_what_is_left() {
    // Through the dispatcher's own renderer, so the arm that reaches these
    // words is the one a run takes rather than one only a test does.
    let said = answer(&Outcome::Wizard(part_way()), false).text();
    assert!(
        said.contains("Where downloads and the library are kept"),
        "{said}"
    );
    assert!(said.contains("How the library is served"), "{said}");
    assert!(
        !said.contains("What applying will write"),
        "a partial plan read as the whole of it: {said}"
    );
}

#[test]
fn a_finished_machine_is_pointed_elsewhere_rather_than_asked_again() {
    let said = standing(&WizardReport {
        offered: false,
        ..part_way()
    })
    .text();
    assert!(said.contains("already set up"), "{said}");
    assert!(!said.contains("still to answer"), "{said}");
}

#[test]
fn an_apply_that_stopped_part_way_is_said_before_anything_else() {
    let said = standing(&WizardReport {
        phase: Phase::Applying,
        ..part_way()
    })
    .text();
    assert!(
        said.starts_with("An earlier apply stopped part-way"),
        "{said}"
    );
}

#[test]
fn an_apply_that_stopped_part_way_names_what_it_had_already_written() {
    // A recovery is chosen about the partial state, so the partial state is
    // what a read of where setup stands has to say.
    let said = standing(&WizardReport {
        phase: Phase::Applying,
        written: vec!["the setting DATA_ROOT".to_owned()],
        ..part_way()
    })
    .text();
    assert!(
        said.contains("it had written: the setting DATA_ROOT"),
        "{said}"
    );
}

#[test]
fn a_complete_set_of_answers_shows_what_applying_would_write() {
    let said = standing(&WizardReport {
        at: Step::Review,
        asks: false,
        unanswered: Vec::new(),
        ready_for_review: true,
        plan: vec![SettingReport {
            key: "INDEXER_APIKEY".to_owned(),
            value: "(set, not shown)".to_owned(),
            secret: true,
            origin: Origin::Operator,
        }],
        ..part_way()
    })
    .text();
    assert!(said.contains("Every question is answered."), "{said}");
    assert!(said.contains("INDEXER_APIKEY=(set, not shown)"), "{said}");
}
