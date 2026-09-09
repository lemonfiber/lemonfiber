//! How a failure is put in front of whoever is reading, and what it exits on.
//!
//! Apart from the codes next door because the two answer different questions. That
//! file decides which number a run leaves with, which is what a script branches on;
//! this decides what a person or a parser is handed on the way out, which is the
//! half that has to be safe to print and the same whichever command produced it.

use std::process::ExitCode;

use crate::render::Lines;
use crate::say::complain;
use lemonfiber_core::error::Problem;
use lemonfiber_core::model::{kind, Envelope};

use super::exit_code;

/// Tell the operator what went wrong, and exit in a way a script can branch on.
///
/// One renderer, so a failure reads the same whichever command produced it —
/// the remedies are the point of the error model, and a second copy of this is
/// how one of them quietly starts omitting them.
pub(crate) fn complain(problem: &Problem) -> ExitCode {
    reported(problem, crate::say::for_a_parser()).eprint();
    ExitCode::from(exit_code(problem))
}

/// What an operator is told about a failure.
///
/// Built rather than printed, which every other answer in this crate already did
/// and this one did not. Three things follow, and only the first was the reason.
///
/// **The text is made safe on the way out.** `Lines::put` passes everything through
/// [`lemonfiber_core::text::plain`], and a failure carries text this product did not
/// write — a service’s own words, a filesystem’s reason. A terminal is not a text
/// box: an escape in the middle of one clears the screen or writes over the line
/// just printed, and a diagnosis that no longer says what this product said has lost
/// the whole of what it was for. The redaction a detail already passed through looks
/// for credentials, not for instructions.
///
/// **A remedy is rendered the one way.** [`Lines::remedy`] exists so that a
/// diagnosis, a repair’s escalation and this cannot drift on how an action and its
/// detail sit together — and this had drifted, by writing the identical shape by
/// hand with nothing keeping the two the same.
///
/// **And a failure explains its own words**, like every other answer, which matters
/// most here: an error is where somebody is least able to go and look one up.
pub(crate) fn reported(problem: &Problem, parsed: bool) -> Lines {
    if parsed {
        return as_a_document(problem);
    }

    let mut lines = Lines::default();
    lines.put(format!("{}: {}", problem.code, problem.summary));
    lines.put("");
    lines.put(format!("  {}", problem.meaning));
    lines.put("");

    for remedy in &problem.remedies {
        lines.remedy(remedy, "  ");
    }

    // Last, and indented: available to whoever wants it, and never the first
    // thing the operator has to read.
    if let Some(detail) = &problem.detail {
        lines.put("");
        for line in detail.lines() {
            lines.put(format!("  {line}"));
        }
    }

    let notes = crate::render::glossary::footnotes(
        &lines.text(),
        crate::render::glossary::wanted(),
        crate::render::glossary::known(),
    );
    lines.extend(notes);
    lines
}

/// The same failure, for something that will parse it.
///
/// One document rather than several lines of prose, and on the error stream still:
/// what a run was asked for goes on standard output, and what went wrong instead
/// belongs beside it rather than in it — a script reading the answer should not have
/// to tell an answer from an apology.
///
/// A script that asked for output it could parse asked about the failures too. They
/// are the answers it most needs to act on, and an exit code alone says that
/// something went wrong without saying what.
fn as_a_document(problem: &Problem) -> Lines {
    let mut lines = Lines::for_a_parser();
    lines.put(
        Envelope::new(kind::ERROR, problem)
            .to_json()
            // Eagerly, for the reason `machine_readable` states beside it: a
            // lazily-built fallback is a line no test could ever run, since these
            // payloads are plain data that cannot fail to serialise.
            .unwrap_or(crate::render::UNRENDERABLE.to_owned()),
    );
    lines
}

/// Where this machine keeps lemonfiber's files.
///
/// Finding the platform's base directories is the surface's job: it means asking
/// the operating system, and there is nothing about it a test could catch that
/// running it would not. The layout beneath those bases is the core's, and is
/// tested there.
/// Refuse an operation that needs to know where the configuration is kept, when
/// this platform will not say. The one message for it, so both callers word it the
/// same way.
pub(crate) fn no_config_home() -> ExitCode {
    complain!("error: lemonfiber could not find where its configuration is kept");
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::reported;
    use lemonfiber_core::error::{Code, Problem, Remedy, Severity};

    /// A failure carries text this product did not write, and a terminal is not a
    /// text box: one escape clears the screen, another writes over the line just
    /// printed. Nothing runs, and the screen stops saying what this product said —
    /// which, for a diagnosis, is the whole of what it was for.
    ///
    /// Every other answer already went out through `text::plain`. This one did not,
    /// and the redaction it *did* pass through looks only for credentials.
    #[test]
    fn a_failure_cannot_carry_an_instruction_to_the_terminal() {
        let escape = char::from(27);
        let problem = Problem::new(
            Code::new("WORD-9"),
            Severity::Error,
            format!("the service refused{escape}[2J"),
            "it gave a reason of its own.",
            Remedy::new("Try again"),
        )
        .with_detail(format!("HTTP 500: {escape}[H database is locked"));

        let said = reported(&problem, false).text();

        assert!(
            !said.contains(escape),
            "an escape reached the screen: {said:?}"
        );
        assert!(
            said.contains("the service refused"),
            "and the words survived: {said}"
        );
        assert!(said.contains("database is locked"), "{said}");
    }

    /// The words a failure uses are explained like any other answer’s, which matters
    /// most here: an error is where somebody is least able to go and look one up.
    #[test]
    fn a_failure_explains_its_own_words() {
        let problem = Problem::new(
            Code::new("WORD-8"),
            Severity::Error,
            "no indexer answered in time",
            "nothing could be searched for.",
            Remedy::new("Check the indexer is reachable"),
        );

        let said = reported(&problem, false).text();

        assert!(said.contains("Words used here:"), "{said}");
        assert!(said.contains("indexer — Search engines"), "{said}");
    }

    /// A script that asked for output it could parse asked about the failures too.
    /// They are the answers it most needs to act on, and an exit code alone says
    /// that something went wrong without saying what.
    ///
    /// Asked of the decision rather than of the latch, so this settles nothing for
    /// the tests beside it.
    #[test]
    fn a_failure_a_script_asked_for_is_one_document_it_can_parse() {
        let problem = Problem::new(
            Code::new("WORD-7"),
            Severity::Error,
            "no indexer answered in time",
            "nothing could be searched for.",
            Remedy::new("Check the indexer is reachable"),
        );

        let said = reported(&problem, true).text();

        assert_eq!(said.lines().count(), 1, "one document, not several: {said}");
        assert!(said.starts_with("{\"api_version\""), "{said}");
        assert!(said.contains("\"kind\":\"error\""), "{said}");
        assert!(said.contains("no indexer answered in time"), "{said}");
        assert!(
            !said.contains("Words used here:"),
            "and nothing a person would want in it: {said}"
        );
    }
}
