//! The escape hatch for a command that has not been taught to rehearse.
//!
//! Its own file, and outside the crate on purpose. `app::rehearsal` decides what
//! `--dry-run` means in a match the compiler checks, and one arm of that match is
//! unreachable through `permitted`: every command in the tree has since been taught,
//! so nothing produces the untaught verdict any more. The arm is shipped all the same,
//! because it is what whoever adds the next command gets — and an escape hatch nobody
//! has ever been through is one nobody knows the shape of.
//!
//! An in-crate test would reach it in the build that carries `cfg(test)` and nowhere
//! else, which leaves the copy that actually ships unentered. From here it is the
//! shipped copy that answers, which is the one an operator would meet.
//!
//! What it has to do is refuse, and refuse under a code of its own: an operator who
//! typed `--dry-run` and was quietly run for real is the failure the whole flag exists
//! to prevent, and "this one has not been taught yet" is a different thing to be told
//! from "this one cannot be rehearsed at all".

use lemonfiber_core::app::rehearsal::not_taught_yet;

/// It refuses, it names the command, and it says the gap is a gap.
#[test]
fn an_untaught_command_is_refused_and_says_so_in_its_own_words() {
    let refusal = not_taught_yet("invent");

    assert!(
        refusal.summary.contains("invent"),
        "a refusal that does not name the command leaves the operator guessing: {}",
        refusal.summary
    );
    assert!(
        refusal.meaning.contains("Nothing was done"),
        "the first thing to say is that nothing happened: {}",
        refusal.meaning
    );
    assert!(
        refusal
            .remedies
            .iter()
            .any(|remedy| remedy.action.contains("without `--dry-run`")),
        "an operator told no is entitled to be told what to do instead: {:?}",
        refusal.remedies
    );
}
