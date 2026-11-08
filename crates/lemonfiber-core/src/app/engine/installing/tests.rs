use super::{held_back, unheld};
use crate::doctor::{Category, Finding, Verdict};
use crate::error::{Problem, Remedy};
use crate::plugin::{Changed, Proving, Verdict as ProofVerdict, Verification};

fn proof(id: &str, came_to: Option<ProofVerdict>) -> Proving {
    Proving {
        proof: id.to_owned(),
        establishes: String::new(),
        of: None,
        asks: "GET /status".to_owned(),
        why: String::new(),
        came_to,
    }
}

fn broke(check: &str, verdict: Verdict) -> Changed {
    Changed {
        now: Finding::in_category(Category::Services, check, "it answers", verdict),
        before: None,
    }
}

#[test]
fn what_held_an_install_back_is_every_proof_that_did_not_hold_and_every_check_it_broke() {
    let proofs = [
        proof("abs.passes", Some(ProofVerdict::Passed)),
        proof(
            "abs.fails",
            Some(ProofVerdict::Failed {
                faults: vec!["status 502".to_owned(), "app is missing".to_owned()],
            }),
        ),
        proof(
            "abs.unrun",
            Some(ProofVerdict::Unproven {
                why: "nothing answered".to_owned(),
            }),
        ),
        proof("abs.unasked", None),
    ];
    let failing = Problem::new(
        crate::error::codes::life::OFFLINE_INSTALL_UNHELD,
        "Jellyfin stopped answering",
        "",
        Remedy::new("Start it"),
    );
    let verified = Verification {
        broke: vec![
            broke("service.jellyfin", Verdict::Fail(failing)),
            broke("service.door", Verdict::Pass { note: None }),
        ],
        unsettled: Vec::new(),
    };

    assert_eq!(
        held_back(&proofs, Some(&verified)),
        [
            "abs.fails: status 502, app is missing",
            "abs.unrun: nothing answered",
            "service.jellyfin: Jellyfin stopped answering",
            "service.door: it answers",
        ]
    );
}

#[test]
fn an_install_that_did_not_hold_says_why_where_it_knows_and_nothing_where_it_does_not() {
    assert_eq!(unheld("audiobookshelf", &[]).detail, None);
    assert_eq!(
        unheld("audiobookshelf", &["abs.fails: status 502".to_owned()]).detail,
        Some("abs.fails: status 502".to_owned())
    );
    assert!(held_back(&[proof("abs.passes", Some(ProofVerdict::Passed))], None).is_empty());
}
