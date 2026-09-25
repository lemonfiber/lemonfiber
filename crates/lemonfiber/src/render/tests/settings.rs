//! Forms, versions and settings as they are rendered.

use super::*;

/// The id first, because that is what gets typed. A form that cannot be combined
/// says so in the listing rather than only when a combination is refused.
#[test]
fn forms_are_listed_by_what_you_would_type_and_what_it_is_for() {
    let text = forms(&some_forms()).text();
    assert!(text.contains("search — Find things."), "{text}");
    assert!(text.contains("everything — The whole stack."), "{text}");
    assert!(text.contains("cannot be combined"), "{text}");
    assert_eq!(
        text.matches("cannot be combined").count(),
        1,
        "only the form that cannot"
    );
}

/// A stack is free to declare none, and a blank screen would read as a broken
/// command rather than as an answer.
#[test]
fn a_stack_with_no_forms_says_so() {
    let text = forms(&FormsReport { forms: Vec::new() }).text();
    assert_eq!(text, "This stack declares no forms.");
}

#[test]
fn versions_name_the_binary_the_stack_and_compose() {
    let text = versions(&a_version()).text();
    assert!(text.contains("stack 1.2.3"));
    assert!(text.contains("compose 2.29"));
    // A compose that could not be reached says so rather than being left blank.
    let unreachable = VersionReport {
        compose: None,
        ..a_version()
    };
    assert!(versions(&unreachable)
        .text()
        .contains("compose not reachable"));
}

/// A proposal over `DATA_ROOT`, standing wherever the test needs it to.
fn proposal(stance: Stance) -> Review {
    Review {
        change: Change {
            key: "DATA_ROOT".to_owned(),
            from: Some("/data".to_owned()),
            to: "/srv/media".to_owned(),
            cost: Cost::Consequential,
        },
        stance,
        refusal: matches!(stance, Stance::Blocked)
            .then(|| "the replacement could not be proven".to_owned()),
        proof: None,
        findings: Findings::default(),
    }
}

/// A report over that proposal, listing the setting it names.
fn proposing(stance: Stance) -> ConfigReport {
    ConfigReport {
        settings: vec![SettingReport {
            key: "DATA_ROOT".to_owned(),
            value: "/data".to_owned(),
            secret: false,
            origin: Origin::Operator,
        }],
        changed: matches!(stance, Stance::Pending | Stance::Applied),
        consequence: None,
        rehearsed: false,
        review: Some(proposal(stance)),
    }
}

/// What a plugin replaced is on the line with what it put there, with whose the
/// replaced value was — and a credential's is never printed.
#[test]
fn an_overridden_setting_says_what_it_replaced_and_an_orphaned_one_whose_it_was() {
    let replaced =
        |value: Option<&str>, withheld: bool, from: Origin| lemonfiber_core::origin::Replaced {
            value: value.map(str::to_owned),
            withheld,
            from: Box::new(from),
        };
    let komga = |replaced| Origin::Overridden {
        named: "komga".to_owned(),
        replaced,
    };
    let text = settings(&listing(vec![
        (
            "E",
            komga(replaced(Some("Europe/Paris"), false, Origin::Operator)),
        ),
        ("F", komga(replaced(None, false, Origin::Bundled))),
        ("G", komga(replaced(None, true, Origin::Operator))),
        (
            "H",
            Origin::Orphaned {
                named: "plex".to_owned(),
            },
        ),
        (
            "I",
            komga(replaced(
                Some("x"),
                false,
                Origin::Unknown {
                    why: "nothing recorded it".to_owned(),
                },
            )),
        ),
    ]))
    .text();
    assert!(text.contains("  nothing recorded it"), "{text}");
    assert!(
        text.contains("E=/data  — set by plugin komga, replacing Europe/Paris (yours)"),
        "{text}"
    );
    assert!(
        text.contains("F=/data  — set by plugin komga, replacing nothing (lemonfiber's own)"),
        "{text}"
    );
    assert!(
        text.contains("G=/data  — set by plugin komga, replacing a withheld value (yours)"),
        "{text}"
    );
    assert!(
        text.contains("H=/data  — left by plugin plex, which is no longer installed"),
        "{text}"
    );
}

/// Every origin reads as words on the line the value is on, because a reader who
/// has to go and look somewhere else is a reader who does not.
#[test]
fn every_setting_says_where_it_came_from_beside_its_value() {
    let text = settings(&listing(vec![
        ("A", Origin::Bundled),
        ("B", Origin::Operator),
        (
            "C",
            Origin::Plugin {
                named: "komga".to_owned(),
            },
        ),
        (
            "D",
            Origin::Unknown {
                why: "no record of writing here".to_owned(),
            },
        ),
    ]))
    .text();
    assert!(text.contains("A=/data  — lemonfiber's own"), "{text}");
    assert!(text.contains("B=/data  — yours"), "{text}");
    assert!(text.contains("C=/data  — set by plugin komga"), "{text}");
    assert!(text.contains("D=/data  — origin unknown"), "{text}");
}

/// The reasons sit under the listing rather than on every line, and each is given
/// once however many settings share it.
#[test]
fn why_an_origin_is_unknown_is_said_once_under_the_listing() {
    let text = settings(&listing(vec![
        (
            "A",
            Origin::Unknown {
                why: "no record of writing here".to_owned(),
            },
        ),
        (
            "B",
            Origin::Unknown {
                why: "no record of writing here".to_owned(),
            },
        ),
        (
            "C",
            Origin::Unknown {
                why: "a credential is never recorded".to_owned(),
            },
        ),
    ]))
    .text();
    assert!(
        text.contains("Where a setting's origin is unknown:"),
        "{text}"
    );
    assert_eq!(
        text.matches("no record of writing here").count(),
        1,
        "a reason shared by two settings is given once: {text}"
    );
    assert!(text.contains("a credential is never recorded"), "{text}");
}

/// The acceptance half. A listing nothing is unknown about carries no block of
/// reasons at all — a rule that only ever adds is one nobody can tell is working.
#[test]
fn a_listing_with_nothing_unknown_says_nothing_about_unknown_origins() {
    let text = settings(&listing(vec![
        ("A", Origin::Bundled),
        ("B", Origin::Operator),
    ]))
    .text();
    assert!(!text.contains("origin unknown"), "{text}");
    assert!(
        !text.contains("Where a setting's origin is unknown"),
        "{text}"
    );
}

#[test]
fn a_change_that_costs_something_says_so_where_it_was_made() {
    // The only moment it is worth saying: the checks deliberately go quiet
    // about it afterwards, so if it is not here the operator never hears it.
    let report = ConfigReport {
        consequence: Some("seeding will be slower".to_owned()),
        ..proposing(Stance::Applied)
    };
    let text = settings(&report).text();
    assert!(text.contains("saved"));
    assert!(text.contains("seeding will be slower"), "{text}");
}

/// The difference is the thing an operator decides on, so it is on the screen
/// whatever became of it — including the run that decided nothing.
#[test]
fn a_proposed_change_shows_what_it_would_replace() {
    let text = settings(&proposing(Stance::Pending)).text();
    assert!(text.contains("DATA_ROOT: /data → /srv/media"), "{text}");
    assert!(
        text.contains("not saved: this change has consequences"),
        "{text}"
    );
    assert!(text.contains("--confirm"), "{text}");
}

/// A setting that has never been written is named as unset rather than left
/// blank, which would read as a value that is the empty string.
#[test]
fn a_setting_with_nothing_in_it_yet_is_named_rather_than_left_blank() {
    let mut fresh = proposal(Stance::Applied);
    fresh.change.from = None;
    let report = ConfigReport {
        review: Some(fresh),
        ..proposing(Stance::Applied)
    };
    let text = settings(&report).text();
    assert!(text.contains("DATA_ROOT: (not set) →"), "{text}");
}

#[test]
fn settings_are_listed_and_a_change_says_whether_it_saved() {
    let report = proposing(Stance::Applied);
    assert!(settings(&report).text().contains("DATA_ROOT=/data"));
    assert!(settings(&report).text().contains("saved"));
    // A rehearsal must not claim it saved.
    let rehearsed = ConfigReport {
        rehearsed: true,
        ..proposing(Stance::Pending)
    };
    assert!(settings(&rehearsed).text().contains("would save"));
    // Nothing proposed, nothing claimed.
    let read = ConfigReport {
        changed: false,
        review: None,
        ..report
    };
    assert!(!settings(&read).text().contains("save"));
}

/// Two ways of writing nothing, and they are not the same thing: one is a
/// setting that already says what was asked for, the other a change the product
/// would not make. An operator told "nothing saved" for both would have no way
/// to tell which happened.
#[test]
fn nothing_written_says_which_of_the_two_reasons_it_was() {
    let same = settings(&proposing(Stance::Unchanged)).text();
    assert!(same.contains("already set to that"), "{same}");

    let refused = settings(&proposing(Stance::Blocked)).text();
    assert!(refused.contains("nothing was changed"), "{refused}");
    assert!(
        refused.contains("the replacement could not be proven"),
        "{refused}"
    );
}

/// An explanation is the word rather than a report that used one, so explaining
/// it again underneath would be the same sentence twice and a pointer at the
/// command that had just been run.
#[test]
fn an_explanation_carries_no_footnote_about_the_word_it_explains() {
    // Held against the switch that would also have emptied it, so a run with
    // explanations turned off could not pass this by explaining nothing at all.
    assert!(super::super::glossary::wanted(), "explanations are on");

    let one = answer(&Outcome::Word(a_term()), false).text();
    assert!(
        one.contains("Search engines"),
        "the word is answered: {one}"
    );
    assert!(!one.contains("Words used here:"), "{one}");

    let listed = answer(
        &Outcome::Glossary(Vocabulary {
            words: vec![a_term()],
        }),
        false,
    )
    .text();
    assert!(!listed.contains("Words used here:"), "{listed}");
}
