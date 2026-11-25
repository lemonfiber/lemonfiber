use lemonfiber_core::app::admitting::{Admitted, Answer, Caller, Calling, SignInCode};
use lemonfiber_core::app::standing::StackStanding;
use lemonfiber_core::app::Outcome;
use lemonfiber_core::device_keys::Challenged;
use lemonfiber_core::instant::Instant;
use lemonfiber_core::keys::Scope;

use super::super::{shaped, Lines};

fn admitted(session: &Admitted) -> Lines {
    shaped(&Outcome::Admitting(Answer::Admission(session.clone())))
}

fn coded(code: &SignInCode) -> Lines {
    shaped(&Outcome::Admitting(Answer::SignInCode(code.clone())))
}

fn calling(caller: &Calling) -> Lines {
    shaped(&Outcome::Admitting(Answer::Caller(caller.clone())))
}

fn challenged(issued: &Challenged) -> Lines {
    shaped(&Outcome::Admitting(Answer::Challenge(issued.clone())))
}

fn at() -> Instant {
    Instant::from_seconds(1_791_180_000)
}

#[test]
fn a_session_is_said_without_its_secret() {
    let session = Admitted {
        token: "s3cr3t".to_owned(),
        until: at(),
        member: Some("m1".to_owned()),
        name: Some("Ada".to_owned()),
    };
    let said = admitted(&session).text();
    assert!(said.contains("Ada is signed in until"), "{said}");
    assert!(!said.contains("s3cr3t"), "{said}");
    let operator = Admitted {
        member: None,
        name: None,
        ..session
    };
    assert!(admitted(&operator).text().starts_with("Signed in until"));
}

#[test]
fn a_code_says_where_to_approve_it_until_it_waits() {
    let asked = SignInCode {
        code: Some("ABCD".to_owned()),
        approve_at: Some("https://example.org/link".to_owned()),
        lapses_at: at(),
        waiting: "h".to_owned(),
    };
    assert!(coded(&asked)
        .text()
        .contains("Approve ABCD at https://example.org/link."));
    let waiting = SignInCode {
        code: None,
        approve_at: None,
        ..asked
    };
    assert!(coded(&waiting).text().contains("still waiting"));
}

#[test]
fn every_answer_about_a_caller_is_said_apart() {
    let answers = [
        Calling::Known {
            caller: Caller::Operator,
        },
        Calling::Known {
            caller: Caller::Member {
                id: "m1".to_owned(),
            },
        },
        Calling::Known {
            caller: Caller::Key {
                name: "ha".to_owned(),
                scope: Scope::Read,
            },
        },
        Calling::Nobody,
        Calling::Unconfirmed,
        Calling::Held { retry_after: 8 },
        Calling::Exposed,
    ];
    let said: std::collections::BTreeSet<String> = answers
        .iter()
        .map(|answer| calling(answer).text())
        .collect();
    assert_eq!(said.len(), answers.len());
    assert!(said.iter().any(|line| line.contains("8 seconds")));
}

#[test]
fn a_challenge_is_said_without_the_challenge() {
    let issued = Challenged {
        challenge: "c4ll3ng3".to_owned(),
        expires: at(),
    };
    let said = challenged(&issued).text();
    assert!(said.contains("stands until"), "{said}");
    assert!(!said.contains("c4ll3ng3"), "{said}");
}

#[test]
fn what_the_web_api_composes_from_is_said_line_by_line() {
    let standing = StackStanding {
        stack: Some("f3a1c2d4".to_owned()),
        switched_off: vec!["LEMONFIBER_REACH_REGISTRY".to_owned()],
        told: true,
        waiting: false,
        members_mint: true,
        plays_from: lemonfiber_core::screening::Origin::of("https://watch.home.lan"),
    };
    let said = shaped(&Outcome::Surfaced(
        lemonfiber_core::app::surfacing::Answer::Standing(standing),
    ))
    .text();
    assert!(said.contains("Stack f3a1c2d4."), "{said}");
    assert!(said.contains("LEMONFIBER_REACH_REGISTRY is off."), "{said}");
    assert!(said.contains("A way to be told: yes."), "{said}");
    assert!(said.contains("A push waiting: no."), "{said}");
    let none = shaped(&Outcome::Surfaced(
        lemonfiber_core::app::surfacing::Answer::Standing(StackStanding::default()),
    ))
    .text();
    assert!(none.contains("No stack identifier is kept here."), "{none}");
}
