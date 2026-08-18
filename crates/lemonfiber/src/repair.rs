//! Putting right what the diagnosis found, at the operator's word.
//!
//! The word is the whole of it. `doctor` looks and changes nothing; `doctor --fix` offers
//! each repair with what it would do and what else changes, and waits to be told; and only
//! `--yes` on the same run carries them out unasked, which is a thing somebody types
//! deliberately rather than a default anybody can fall into.
//!
//! What may be offered, what has been declined and what has been tried too often are all
//! the core's to decide. What is here is the asking.

use std::process::ExitCode;

use lemonfiber_core::app::repair::mend;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::repair::{Repair, Stance};

use crate::cli::Mending;
use crate::prompt::{yes_no, Answers};
use crate::render::repair::mended;
use crate::render::Lines;

/// Offer the repairs, and carry out the ones agreed to.
pub(crate) async fn run(ctx: Ctx, asked: Mending, answers: &dyn Answers, json: bool) -> ExitCode {
    // Nobody is there to answer a prompt in machine-readable mode, and a script that wanted
    // repairs carried out says so with --yes. So one that did not gets the offer and no
    // action, which is what report-only is for.
    let stance = match (asked.yes, json) {
        (true, _) => Stance::Unattended,
        (false, true) => Stance::ReportOnly,
        (false, false) => Stance::Ask,
    };

    match mend(&ctx, stance, asked.disruptive, |repair| {
        agreed(repair, answers)
    })
    .await
    {
        Ok(report) => {
            mended(&report, json).print();
            crate::exit::repairing(&report)
        }
        Err(problem) => crate::complain(&problem),
    }
}

/// Ask about one repair, having said what it would do and what else changes.
///
/// Stated before the question rather than after it, because an effect somebody learns about
/// afterwards is not something they agreed to.
fn agreed(repair: &Repair, answers: &dyn Answers) -> bool {
    stated(repair).print();
    yes_no(answers, &format!("{}?", repair.does), false)
}

/// What is about to be agreed to, built as lines like every other answer this binary
/// gives rather than printed where it is decided — the terminal is reached in one place.
fn stated(repair: &Repair) -> Lines {
    let mut lines = Lines::default();
    for effect in &repair.effects {
        lines.put(format!("  {effect}"));
    }
    if !repair.reversible {
        lines.put("  This one cannot be undone.");
    }
    lines
}

#[cfg(test)]
mod tests {
    use lemonfiber_core::app::repair::{Mended, Report};
    use lemonfiber_core::repair::{Outcome, Repair};

    use std::sync::Arc;

    use lemonfiber_core::config::Settings;
    use lemonfiber_core::platform::Environment;
    use lemonfiber_core::stack::Source;

    use crate::exit::{repairing, shown, success};
    use crate::prompt::Answers;

    use super::{agreed, run, Ctx, Mending};

    /// A context over the stack this binary ships, with nothing configured — so nothing is
    /// wrong that lemonfiber could put right, which is the state a healthy machine is in.
    fn ctx() -> Ctx {
        Ctx::new(
            Arc::new(lemonfiber_core::adapters::Local),
            Arc::new(lemonfiber_core::adapters::Daemon::local()),
            Arc::new(lemonfiber_core::adapters::System),
            Arc::new(lemonfiber_core::adapters::Disk),
            Source::Embedded(&crate::cli::STACK),
            Settings::default(),
            Environment::MacOs,
        )
    }

    /// Answers whatever it was built with, however often it is asked.
    struct Says(&'static str);

    impl Answers for Says {
        fn ask(&self, _question: &str) -> String {
            self.0.to_owned()
        }
        fn secret(&self, _prompt: &str) -> String {
            String::new()
        }
    }

    fn repair(reversible: bool) -> Repair {
        Repair {
            check: "vpn.port-forward-client".to_owned(),
            does: "Move the download client onto the forwarded port".to_owned(),
            effects: vec!["Transfers in flight pause briefly".to_owned()],
            reversible,
        }
    }

    fn report(outcomes: Vec<Outcome>) -> Report {
        Report {
            offered: Vec::new(),
            beyond: Vec::new(),
            mended: outcomes
                .into_iter()
                .map(|outcome| Mended {
                    repair: repair(true),
                    outcome,
                })
                .collect(),
            acted: true,
        }
    }

    /// Only yes means yes. Anything else — including the empty answer of somebody
    /// pressing return to make the question go away — leaves the machine alone.
    #[test]
    fn nothing_but_yes_agrees_to_a_repair() {
        assert!(agreed(&repair(true), &Says("y")));
        assert!(agreed(&repair(true), &Says("YES")));
        assert!(!agreed(&repair(true), &Says("n")));
        assert!(!agreed(&repair(true), &Says("")));
        // One that cannot be undone says so before the question, and is still only
        // carried out on a yes.
        assert!(agreed(&repair(false), &Says("y")));
    }

    /// The command end to end, over a stack with nothing wrong that lemonfiber can mend.
    ///
    /// Every stance reaches the same answer here — there is nothing to offer, so there is
    /// nothing to ask about and nothing to carry out — which is what a run on a healthy
    /// machine should say however it was asked.
    #[tokio::test]
    async fn a_run_with_nothing_to_mend_succeeds_however_it_was_asked() {
        let asking = |fix: bool, yes: bool, json: bool| async move {
            shown(
                run(
                    ctx(),
                    Mending {
                        fix,
                        yes,
                        disruptive: false,
                    },
                    &Says("n"),
                    json,
                )
                .await,
            )
        };

        // Asked about each, told to go ahead, and read by a script: all three.
        assert_eq!(asking(true, false, false).await, success());
        assert_eq!(asking(true, true, false).await, success());
        assert_eq!(asking(true, false, true).await, success());
    }

    /// An operator who asked for things to be put right and had one fail needs their
    /// script to know, and a run with nothing to mend has mended everything it offered.
    #[test]
    fn anything_left_unmended_is_a_non_zero_result() {
        assert_eq!(shown(repairing(&report(Vec::new()))), success());
        assert_eq!(shown(repairing(&report(vec![Outcome::Fixed]))), success());
        assert_ne!(
            shown(repairing(&report(vec![Outcome::FixFailed]))),
            success()
        );
        assert_ne!(
            shown(repairing(&report(vec![Outcome::Declined]))),
            success()
        );
        assert_ne!(
            shown(repairing(&report(vec![Outcome::Fixed, Outcome::FixFailed]))),
            success()
        );
    }
}
