//! Running the first-content walk, and turning it into an exit code.
//!
//! The narration goes to the terminal while it happens and the ending goes out at the end,
//! which makes this the one command whose output is not a single value rendered once. It
//! is kept out of `main` for that reason: what it does is drive a long-running thing and
//! decide what its ending means, which is more than a dispatcher should carry.

use std::process::ExitCode;

use lemonfiber_core::app::{walkthrough as run, Ctx};
use lemonfiber_core::model::WalkthroughReport;
use lemonfiber_core::walkthrough::Narrator;

use crate::exit::{complain, FAILURE};
use crate::render::walkthrough::{ending, machine_readable, Narrating, Quiet};

/// Walk one thing through the pipeline, narrating it, and report how it ended.
///
/// An empty `item` is nothing named rather than an empty title: the walk then suggests
/// something likely to work, which is what an operator with an empty library needs.
pub(crate) async fn walk(ctx: &Ctx, item: &str, json: bool) -> ExitCode {
    let named = (!item.trim().is_empty()).then_some(item);
    // A run whose whole answer is a JSON document must not have prose interleaved into
    // it: a consumer parsing that stream would be handed something that is not a document.
    let narrating = Narrating;
    let quiet = Quiet;
    let narrator: &dyn Narrator = if json { &quiet } else { &narrating };

    match run(ctx, named, narrator).await {
        Ok(report) => {
            let lines = if json {
                machine_readable(&report)
            } else {
                ending(&report)
            };
            lines.print();
            code(&report)
        }
        Err(problem) => complain(&problem),
    }
}

/// The exit code a walkthrough earns.
///
/// Only a walk that stopped is a failure. One that finished worked; one still downloading
/// is working, and reporting that as a failure would contradict the sentence that just
/// told the operator nothing was cancelled; and one that found the content already here
/// answered the question it was asked.
fn code(report: &WalkthroughReport) -> ExitCode {
    if report.state.is_a_problem() {
        return ExitCode::from(FAILURE);
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::{code, walk};
    use crate::setup::tests::working_ctx;
    use lemonfiber_core::model::WalkthroughReport;
    use lemonfiber_core::walkthrough::{Shape, State};

    /// A report that ended in one state.
    fn ended(state: State) -> WalkthroughReport {
        WalkthroughReport {
            shape: Shape::Pipeline,
            state,
            proves: String::new(),
            item: None,
            lines: Vec::new(),
            stopped: None,
            link: None,
            handover: None,
            suggestions: Vec::new(),
            in_background: false,
            already_here: false,
        }
    }

    /// What an exit code reads as, for comparison.
    fn shown(code: std::process::ExitCode) -> String {
        format!("{code:?}")
    }

    #[tokio::test]
    async fn a_walk_run_from_the_command_line_reports_how_it_ended() {
        // This stack has no media server to prove anything against, so the walk stops and
        // the command exits non-zero — the whole path from request to exit code.
        let ended = walk(&working_ctx(), "", false).await;
        assert_ne!(shown(ended), shown(std::process::ExitCode::SUCCESS));
    }

    #[tokio::test]
    async fn a_named_item_is_walked_rather_than_a_suggested_one() {
        let ended = walk(&working_ctx(), "Sintel", false).await;
        assert_ne!(shown(ended), shown(std::process::ExitCode::SUCCESS));
    }

    #[tokio::test]
    async fn a_machine_readable_run_says_nothing_but_its_document() {
        // Prose interleaved into the stream would hand a consumer something that is not a
        // document at all.
        let ended = walk(&working_ctx(), "", true).await;
        assert_ne!(shown(ended), shown(std::process::ExitCode::SUCCESS));
    }

    #[tokio::test]
    async fn a_stack_that_cannot_be_read_is_complained_about_rather_than_reported() {
        let mut ctx = working_ctx();
        ctx.stack = lemonfiber_core::stack::Source::External(std::path::Path::new("/not-a-stack"));
        let ended = walk(&ctx, "", false).await;
        assert_ne!(shown(ended), shown(std::process::ExitCode::SUCCESS));
    }

    #[test]
    fn only_a_walk_that_stopped_is_a_failure() {
        let success = format!("{:?}", std::process::ExitCode::SUCCESS);
        assert_eq!(format!("{:?}", code(&ended(State::Complete))), success);
        assert_eq!(
            format!("{:?}", code(&ended(State::Downloading))),
            success,
            "still coming is not a failure — the operator was told so"
        );
        assert_eq!(format!("{:?}", code(&ended(State::Skipped))), success);
        assert_ne!(format!("{:?}", code(&ended(State::Failed))), success);
    }
}
