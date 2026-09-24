use std::path::Path;

use super::{reading, write};
use crate::app::fixtures::FakeArchive;
use crate::bundle::{Contents, Piece, Taken, Terms, MANIFEST};
use crate::doctor::Verdict;
use crate::error::{Code, Problem, Remedy, Severity};

/// Naming a field and agreeing to publish it are two acts. The refusal names what
/// would have been shown, so what gets confirmed is those settings rather than a
/// policy somebody agrees to once and forgets.
#[test]
fn showing_a_setting_as_it_is_has_to_be_said_twice() {
    let refused = super::unconfirmed(&["SONARR_API_KEY".to_owned(), "X".to_owned()]);
    assert_eq!(refused.code, super::BUNDLE_UNCONFIRMED);
    assert_eq!(
        refused.detail.as_deref(),
        Some("would have shown: SONARR_API_KEY, X")
    );
}

/// A machine that will not produce random bytes gets no bundle rather than one whose
/// stand-ins anybody can reproduce — and is told which of those two it is.
#[test]
fn a_machine_that_offers_no_randomness_is_told_why_it_gets_no_bundle() {
    let refused = super::without_marks();
    assert_eq!(refused.code, super::BUNDLE_NO_MARKS);
    assert!(refused.summary.contains("could not be made"));
}

/// A problem as a check reports one.
fn problem() -> Problem {
    Problem::new(
        Code::new("BUNDLE-0"),
        Severity::Warning,
        "something is wrong",
        "why it matters",
        Remedy::new("do something"),
    )
}

/// Every verdict a check can reach, in the words the check chose. A bundle that
/// paraphrased any of them would leave the operator and the person helping comparing
/// two accounts of one finding.
#[test]
fn a_verdict_reads_in_the_words_the_check_used() {
    assert_eq!(
        reading(&Verdict::Pass {
            note: Some("340 GiB left".to_owned())
        }),
        "340 GiB left"
    );
    // A pass with nothing to add says nothing rather than inventing a reassurance.
    assert_eq!(reading(&Verdict::Pass { note: None }), "");
    assert_eq!(reading(&Verdict::Warn(problem())), "something is wrong");
    assert_eq!(reading(&Verdict::Fail(problem())), "something is wrong");
    assert_eq!(
        reading(&Verdict::Unverified {
            reason: "nothing answered".to_owned(),
            remedy: Remedy::new("try again"),
        }),
        "nothing answered"
    );
    assert_eq!(
        reading(&Verdict::Skipped {
            reason: "nothing to read".to_owned()
        }),
        "nothing to read"
    );
}

/// The archive is handed the whole bundle at once, at the path it was asked for, and
/// what comes back names every file in it — the bundle's own first page included.
///
/// The operator is told what they are about to attach before anybody else reads it,
/// so what `write` reports has to be what the archive was actually given rather than
/// what the collector meant to give it.
#[tokio::test]
async fn a_bundle_is_handed_to_the_archive_whole() {
    let contents = Contents {
        pieces: vec![Piece {
            name: "platform.txt".to_owned(),
            body: "lemonfiber 0.7.0".to_owned(),
        }],
        missing: Vec::new(),
        taken: Taken {
            lemonfiber: "0.7.0".to_owned(),
            stack: "1.2.0".to_owned(),
            at: "2026-08-18T00:00:00Z".to_owned(),
        },
        terms: Terms::default(),
    };
    let archive = FakeArchive::roomy();
    let dest = Path::new("/tmp/support.tar.gz");

    let written = write(&archive, &contents, dest).await;

    assert_eq!(
        written.map(|written| (written.path, written.holds)),
        Ok((
            dest.to_path_buf(),
            vec![MANIFEST.to_owned(), "platform.txt".to_owned()]
        ))
    );
    assert_eq!(archive.writes(), vec![dest.to_path_buf()]);
}
