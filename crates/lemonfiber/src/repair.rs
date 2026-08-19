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

use lemonfiber_core::app::repair::{mend, Confirm};
use lemonfiber_core::app::Ctx;
use lemonfiber_core::repair::{Repair, Stance};

use lemonfiber_core::app::recover;
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::repair;

use crate::cli::Mending;
use crate::prompt::{yes_no, Answers};
use crate::render::repair::{mended, reversed};
use crate::render::Lines;

/// Offer the repairs and carry out the ones agreed to, or put back what the last one did.
pub(crate) async fn run(
    ctx: Ctx,
    paths: Paths,
    asked: Mending,
    answers: &dyn Answers,
    json: bool,
) -> ExitCode {
    if asked.undo {
        return undone(&paths, json);
    }
    // Nobody is there to answer a prompt in machine-readable mode, and a script that wanted
    // repairs carried out says so with --yes. So one that did not gets the offer and no
    // action, which is what report-only is for.
    let stance = match (asked.yes, json) {
        (true, _) => Stance::Unattended,
        (false, true) => Stance::ReportOnly,
        (false, false) => Stance::Ask,
    };

    match mend(&ctx, stance, asked.disruptive, &Asking(answers)).await {
        Ok(report) => {
            mended(&report, json).print();
            crate::exit::repairing(&report)
        }
        Err(problem) => crate::complain(&problem),
    }
}

/// Put back what the last repair changed.
///
/// That repair and no other. The journal it reads is shared with seeding and the first-run
/// wizard, and somebody undoing the thing they just watched happen has not asked for the
/// wiring underneath it to come apart.
fn undone(paths: &Paths, json: bool) -> ExitCode {
    let journal = recover::journal_at(&paths.journal());
    let undos = repair::undoing(journal.changes());
    if undos.is_empty() {
        reversed(&undos, json).print();
        return ExitCode::SUCCESS;
    }
    if let Err(problem) = recover::undo(&undos, &paths.env_file()) {
        return crate::complain(&problem);
    }
    reversed(&undos, json).print();
    ExitCode::SUCCESS
}

/// Asking whoever is at the terminal.
struct Asking<'a>(&'a dyn Answers);

impl Confirm for Asking<'_> {
    /// Ask about one repair, having said what it would do and what else changes.
    ///
    /// Stated before the question rather than after it, because an effect somebody learns
    /// about afterwards is not something they agreed to.
    fn agreed(&self, repair: &Repair) -> bool {
        stated(repair).print();
        yes_no(self.0, &format!("{}?", repair.does), false)
    }
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

    use super::{run, Asking, Confirm as _, Ctx, Mending};

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
    ///
    /// A repair asks one kind of question and only ever that kind, so both halves of the
    /// trait answer the same way — there is no secret to give and nothing that would ask
    /// for one.
    struct Says(&'static str);

    impl Answers for Says {
        fn ask(&self, _question: &str) -> String {
            self.0.to_owned()
        }

        fn secret(&self, question: &str) -> String {
            self.ask(question)
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
    ///
    /// Asked through both halves of the answering trait, because a surface that reached
    /// for a secret would be doing something a repair has no business doing, and the fake
    /// would answer it the same way rather than hiding it.
    #[test]
    fn nothing_but_yes_agrees_to_a_repair() {
        assert!(Asking(&Says("y")).agreed(&repair(true)));
        assert!(Asking(&Says("YES")).agreed(&repair(true)));
        assert!(!Asking(&Says("n")).agreed(&repair(true)));
        assert!(!Asking(&Says("")).agreed(&repair(true)));
        // One that cannot be undone says so before the question, and is still only
        // carried out on a yes.
        assert!(Asking(&Says("y")).agreed(&repair(false)));
        assert_eq!(Says("y").secret("anything"), "y");
    }

    /// A stack that will not read is the one thing every check needs before any of them
    /// can run, so the operator hears about it rather than being told there is nothing to
    /// put right — which would be true of a machine lemonfiber cannot see at all.
    #[tokio::test]
    async fn a_stack_that_cannot_be_read_is_refused_rather_than_called_healthy() {
        let nowhere = Ctx::new(
            Arc::new(lemonfiber_core::adapters::Local),
            Arc::new(lemonfiber_core::adapters::Daemon::local()),
            Arc::new(lemonfiber_core::adapters::System),
            Arc::new(lemonfiber_core::adapters::Disk),
            Source::External(std::path::Path::new("/no/such/stack")),
            Settings::default(),
            Environment::MacOs,
        );

        let code = run(
            nowhere,
            Mending {
                fix: true,
                yes: true,
                disruptive: false,
            },
            &Says("n"),
            false,
        )
        .await;

        assert_ne!(shown(code), success());
    }

    /// The command end to end, over a stack with nothing wrong that lemonfiber can mend.
    ///
    /// Every stance reaches the same answer here — there is nothing to offer, so there is
    /// nothing to ask about and nothing to carry out — which is what a run on a healthy
    /// machine should say however it was asked.
    #[tokio::test]
    async fn a_run_with_nothing_to_mend_succeeds_however_it_was_asked() {
        // Asked about each, told to go ahead, and read by a script: all three reach the
        // same answer on a machine with nothing wrong that lemonfiber could put right.
        assert_eq!(asked_for(false, false).await, success());
        assert_eq!(asked_for(true, false).await, success());
        assert_eq!(asked_for(false, true).await, success());
    }

    /// One run of the command, as the flags would arrive.
    ///
    /// A named function rather than a closure: a closure is a function of its own as far
    /// as coverage is concerned, and one inside a test is one the gate counts and no other
    /// test enters.
    async fn asked_for(yes: bool, json: bool) -> String {
        shown(
            run(
                ctx(),
                Mending {
                    fix: true,
                    yes,
                    disruptive: false,
                },
                &Says("n"),
                json,
            )
            .await,
        )
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
