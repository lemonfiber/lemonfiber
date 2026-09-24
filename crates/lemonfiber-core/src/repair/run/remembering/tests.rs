use super::wrong;
use crate::doctor::{Category, Finding, Verdict};
use crate::error::{Code, Problem, Remedy, Severity};

const CODE: Code = Code::new("storage.full");

/// A finding whose verdict carries the full four parts a problem is made of.
fn found(verdict: Verdict) -> Finding {
    Finding {
        check: "storage.space".to_owned(),
        category: Category::Storage,
        title: "room on the volume".to_owned(),
        verdict,
        service: None,
        caused_by: None,
        said: None,
        origin: crate::origin::Origin::Bundled,
    }
}

/// The problem a full volume amounts to.
fn full() -> Problem {
    Problem::new(
        CODE,
        Severity::Error,
        "the volume has no room left",
        "nothing can be written, so imports will fail where they stand",
        Remedy::new("delete something, or move the library"),
    )
}

#[test]
fn what_a_finding_meant_survives_into_the_condition_it_raises() {
    // The verdict states what happened, what it means and what to do; the
    // condition is what every later surface reads. Keeping two of the three
    // here is how a doctor screen and a dashboard come to disagree about the
    // same problem.
    let remembered = wrong(&found(Verdict::Fail(full())));
    let parts = remembered.map(|fault| (fault.summary, fault.meaning));
    assert_eq!(
        parts,
        Some((
            "the volume has no room left".to_owned(),
            "nothing can be written, so imports will fail where they stand".to_owned(),
        ))
    );
}

#[test]
fn a_check_that_found_nothing_wrong_remembers_nothing() {
    // A pass and a skip say nothing is wrong and nothing was looked at, and an
    // unverified check found no fault — only that it could not say.
    for verdict in [
        Verdict::Pass { note: None },
        Verdict::Skipped {
            reason: "no volume configured".to_owned(),
        },
        Verdict::Unverified {
            reason: "the volume could not be read".to_owned(),
            remedy: Remedy::new("check the path exists"),
        },
    ] {
        assert!(wrong(&found(verdict)).is_none());
    }
}
