//! The findings both halves of the doctor's own tests are written against.
//!
//! Shared because the vocabulary and the run are proven against the same shapes — one
//! asks what a verdict puts on the wire, the other what a list of them sums to — and a
//! fixture copied per module is a fixture that drifts per module.

use super::{Category, Finding, Verdict};
use crate::error::{Code, Problem, Remedy, Severity};

/// The code every diagnosis built here carries.
const CODE: Code = Code::new("TEST-1");

/// A finding of one family, under an identifier no narrowing here is written against.
pub(super) fn finding(title: &str, verdict: Verdict) -> Finding {
    Finding {
        check: "test".to_owned(),
        category: Category::Vpn,
        title: title.to_owned(),
        service: None,
        caused_by: None,
        said: None,
        verdict,
    }
}

/// A diagnosis carrying every field the two verdicts that hold one write.
pub(super) fn problem() -> Problem {
    Problem::new(
        CODE,
        Severity::Error,
        "It broke",
        "The thing did not happen",
        Remedy::new("Try again"),
    )
}
