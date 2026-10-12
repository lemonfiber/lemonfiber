use lemonfiber_core::plugin::{Recorded, Taken, Took};

use super::recorded;

fn read(runs: Vec<Taken>, holds: bool) -> Recorded {
    Recorded {
        id: "quality".to_owned(),
        refusals: Vec::new(),
        runs,
        holds,
    }
}

fn taken(took: Took) -> Taken {
    Taken {
        proof: "recyclarr.runs".to_owned(),
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
fn each_run_proof_is_said_with_what_recording_it_came_to() {
    let every = read(
        vec![
            taken(Took::Written),
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
        "quality — its run proofs, recorded by running them",
        "recordings/recyclarr/runs-version.json: written",
        "reproduced, and left as it was",
        "refused: it differs",
        "still running at its limit of 30 seconds, and stopped",
        "not recorded: [2Jno engine",
        "Not every run proof is recorded",
    ] {
        assert!(said.contains(words), "{words}: {said}");
    }
    assert!(!said.contains('\u{1b}'));
}

#[test]
fn a_plugin_recording_everything_or_nothing_says_so() {
    assert!(shown(&read(vec![taken(Took::Written)], true)).contains("Every run proof is recorded."));
    assert!(shown(&read(Vec::new(), true)).contains("nothing to record"));
    let refused = Recorded {
        refusals: vec![lemonfiber_core::plugin::Violation {
            location: "proof a".to_owned(),
            message: "is wrong".to_owned(),
        }],
        ..read(Vec::new(), false)
    };
    assert!(
        shown(&refused).contains("Refused, 1 violation, so nothing was run:\n  proof a — is wrong")
    );
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
