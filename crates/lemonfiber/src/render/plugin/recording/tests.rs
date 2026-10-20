use lemonfiber_core::plugin::{Failed, Recorded, Taken, Took};

use super::recorded;

fn read(recordings: Vec<Taken>, holds: bool) -> Recorded {
    Recorded {
        id: "quality".to_owned(),
        refusals: Vec::new(),
        recordings,
        failed: Vec::new(),
        holds,
    }
}

fn taken(took: Took) -> Taken {
    Taken {
        named: "proof recyclarr.runs".to_owned(),
        fixture: Some("recordings/recyclarr/runs-version.json".to_owned()),
        took,
    }
}

fn shown(read: &Recorded) -> String {
    recorded(read, false)
        .map(|lines| lines.text())
        .unwrap_or_default()
}

#[test]
fn each_recording_is_said_with_what_taking_it_came_to() {
    let every = read(
        vec![
            taken(Took::Written),
            taken(Took::Unnoted),
            taken(Took::Reproduced),
            taken(Took::Unreproduced {
                why: "it differs".to_owned(),
            }),
            taken(Took::Overran { seconds: 30 }),
            taken(Took::Unrecorded {
                why: "\u{1b}[2Jno engine".to_owned(),
            }),
        ],
        false,
    );
    let said = shown(&every);
    for words in [
        "quality — every recording its manifest names, taken again from its pin",
        "proof recyclarr.runs",
        "recordings/recyclarr/runs-version.json: written",
        "written with an empty note, which has to say why it is evidence",
        "reproduced, and left as it was",
        "refused: it differs",
        "still running at its limit of 30 seconds, and stopped",
        "not recorded: [2Jno engine",
        "Not every recording is taken.",
    ] {
        assert!(said.contains(words), "{words}: {said}");
    }
    assert!(!said.contains('\u{1b}'));
}

#[test]
fn a_plugin_recording_everything_or_nothing_says_so() {
    assert!(shown(&read(vec![taken(Took::Written)], true)).contains("Every recording is taken."));
    assert!(shown(&read(Vec::new(), true))
        .contains("It names no recording, so there is nothing to take."));
    let refused = Recorded {
        refusals: vec![lemonfiber_core::plugin::Violation {
            location: "proof a".to_owned(),
            message: "is wrong".to_owned(),
        }],
        ..read(Vec::new(), false)
    };
    assert!(shown(&refused)
        .contains("Refused, 1 violation, so nothing was taken:\n  proof a — is wrong"));
}

#[test]
fn the_machine_form_is_the_report_as_a_document() {
    let said = recorded(
        &read(vec![taken(Took::Overran { seconds: 5 })], false),
        true,
    )
    .map(|lines| lines.text())
    .unwrap_or_default();
    assert!(said.contains("\"outcome\": \"overran\""), "{said}");
    assert!(said.contains("\"seconds\": 5"), "{said}");
}

#[test]
fn a_service_that_failed_is_said_with_its_step_why_and_the_last_lines_it_wrote() {
    let failing = Recorded {
        failed: vec![Failed {
            service: "sonarr".to_owned(),
            step: "health".to_owned(),
            why: "it exited 3 before its health passed".to_owned(),
            log: vec!["starting".to_owned(), "crashed".to_owned()],
        }],
        ..read(Vec::new(), false)
    };
    let said = shown(&failing);
    for words in [
        "sonarr failed at health: it exited 3 before its health passed. None of its recordings was written.",
        "  The last 2 lines it wrote:\n    starting\n    crashed",
    ] {
        assert!(said.contains(words), "{words}: {said}");
    }
}

#[test]
fn a_service_that_failed_before_it_wrote_anything_is_said_without_lines() {
    let failed = Recorded {
        failed: vec![Failed {
            service: "sonarr".to_owned(),
            step: "container".to_owned(),
            why: "no engine".to_owned(),
            log: Vec::new(),
        }],
        ..read(Vec::new(), false)
    };
    let said = shown(&failed);
    assert!(
        said.contains("sonarr failed at container: no engine. None of its recordings was written."),
        "{said}"
    );
    assert!(!said.contains("lines it wrote"), "{said}");
}
