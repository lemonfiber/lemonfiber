use lemonfiber_core::plugin::{Conformed, Judged, Verdict, Violation};

use super::conformed;

fn case(contract: &str, case: &str, verdict: Verdict) -> Judged {
    Judged {
        contract: contract.to_owned(),
        case: case.to_owned(),
        recording: format!("conformance/{contract}/{case}.json"),
        verdict,
    }
}

fn read(refusals: Vec<Violation>, cases: Vec<Judged>, conforms: bool) -> Conformed {
    Conformed {
        id: "subber".to_owned(),
        refusals,
        cases,
        conforms,
    }
}

fn said(read: &Conformed) -> String {
    conformed(read, false)
        .map(|lines| lines.text())
        .unwrap_or_default()
}

#[test]
fn each_case_is_said_under_its_contract_and_the_whole_is_judged() {
    let passing = read(
        Vec::new(),
        vec![
            case(
                "subtitles.fetch@1",
                "refuses-without-the-key",
                Verdict::Passed,
            ),
            case("subtitles.fetch@1", "watch-answers", Verdict::Passed),
        ],
        true,
    );
    let text = said(&passing);
    assert_eq!(text.matches("subtitles.fetch@1").count(), 1, "{text}");
    for expected in [
        "subber — what its recordings say about the contracts it speaks",
        "watch-answers",
        "the recording answers it",
        "It conforms to every contract it speaks.",
    ] {
        assert!(
            text.contains(expected),
            "{expected:?} missing from:\n{text}"
        );
    }

    let failing = read(
        vec![Violation {
            location: "service subber-adapter.fronts".to_owned(),
            message: "is absent".to_owned(),
        }],
        vec![case(
            "subtitles.fetch@1",
            "watch-answers",
            Verdict::Unproven {
                why: "could not be read".to_owned(),
            },
        )],
        false,
    );
    let text = said(&failing);
    for expected in [
        "Refused, 1 violation:",
        "service subber-adapter.fronts — is absent",
        "unproven: could not be read",
        "It does not conform",
    ] {
        assert!(
            text.contains(expected),
            "{expected:?} missing from:\n{text}"
        );
    }

    assert!(said(&read(Vec::new(), Vec::new(), true))
        .contains("It speaks no contract, so there is nothing to judge."));
    assert!(conformed(&passing, true)
        .map(|lines| lines.text())
        .is_some_and(|json| json.contains("\"conforms\": true")));
}
