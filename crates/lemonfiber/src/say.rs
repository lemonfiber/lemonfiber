//! Everything this product prints, and the one place it decides how.
//!
//! Output used to leave through thirty-odd `println!` calls and a report renderer,
//! which meant a question about *how* something is shown had thirty-odd answers —
//! or, in practice, none. `NO_COLOR` had to be threaded to the one place that used
//! colour; the next such question would have been threaded somewhere else again.
//!
//! So every line goes out through here, and questions about rendering get asked
//! once. Today there is one: whether this terminal can show more than ASCII.
//!
//! **Folding is decided from the locale, and only where it says so.** A locale
//! naming a non-UTF-8 charset, or the `C`/`POSIX` locale, is a terminal that has
//! told us what it can do. A locale that is simply unset has told us nothing, and
//! guessing ASCII there would degrade the ordinary case to serve a rare one — the
//! requirement asks for a fallback where Unicode is *unsupported*, not wherever it
//! is unproven.

use std::io::Write as _;
use std::sync::OnceLock;

/// Whether output is folded to ASCII, settled once at startup.
///
/// A `OnceLock` because it is a property of the terminal this process was given,
/// not of any call: it cannot change while the program runs, and threading it
/// through every line that might eventually be printed would put it in signatures
/// that have nothing to do with it.
static ASCII_ONLY: OnceLock<bool> = OnceLock::new();

/// Settle how output is rendered, and say what is in force.
///
/// The environment is read by the caller rather than here, because the caller is
/// the edge: what a locale *means* is decided below and can be tested, and only
/// the edge knows where the locale came from. It is the same division the log
/// viewer already makes over `NO_COLOR`.
///
/// The first call decides. A later one is ignored and told what the first settled,
/// so what comes back is what is actually in force rather than what this caller
/// asked for — a surface cannot change what a terminal can do halfway through
/// printing to it, and it should not be told that it did.
pub(crate) fn settle(locale: Option<&str>) -> bool {
    *ASCII_ONLY.get_or_init(|| !unicode(locale))
}

/// Whether a terminal described this way can show more than ASCII.
///
/// Read from the charset a locale names. `C` and `POSIX` are the two that say
/// plainly that it cannot; anything naming UTF-8 says it can; and anything else
/// names a charset that is not UTF-8, which is the same answer as `C` for these
/// purposes. Nothing at all is not an answer, and is taken as no objection.
pub(crate) fn unicode(locale: Option<&str>) -> bool {
    let Some(said) = locale.filter(|said| !said.is_empty()) else {
        return true;
    };
    let said = said.to_ascii_uppercase();
    if said.contains("UTF-8") || said.contains("UTF8") {
        return true;
    }
    // A locale that names a charset has named one that is not UTF-8, and `C` and
    // `POSIX` name the minimal one outright. Either way, it has told us.
    !said.contains('.') && said != "C" && said != "POSIX"
}

/// The line as this terminal can render it.
///
/// Reachable from outside because not everything that reaches a terminal does so
/// through this module: reading a secret hands the prompt to a crate that writes it
/// itself, and that prompt has to arrive folded like every other line.
///
/// Made plain as well as folded, and here rather than at the call sites. A report
/// built out of `Lines` is made plain as each line goes in, but a stream has no
/// report to build: Compose's own output and a walkthrough's narration are
/// printed the moment they arrive, and both of those are somebody else's text.
/// Neither had been, so `\x1b[2J` in a release title cleared the operator's terminal
/// midway through a walkthrough. A rule that has to be remembered at each of thirty
/// call sites is a rule that will be forgotten at one, so it is applied at the one
/// place they all pass through.
pub(crate) fn rendered(line: &str) -> String {
    shown(&drawable(line), *ASCII_ONLY.get().unwrap_or(&false))
}

/// Every line of what one call prints, with anything a terminal would obey removed.
///
/// Line by line, because the breaks are ours and the rest of it is not. A caller
/// asks for a blank line before its heading by writing one into the text, and
/// `plain` — which is built for one line of a report — would take it away along
/// with the escape it is really there to remove.
fn drawable(text: &str) -> String {
    text.split('\n')
        .map(lemonfiber_core::text::plain)
        .collect::<Vec<_>>()
        .join("\n")
}

/// The line as a terminal of this kind can render it.
///
/// Takes the answer rather than looking it up, so what is decided here can be
/// tested without settling a value that outlives the test — the lookup is a
/// process-wide latch, and a test that tripped it would decide for every test
/// after it.
pub(crate) fn shown(line: &str, ascii_only: bool) -> String {
    if ascii_only {
        return folded(line);
    }
    line.to_owned()
}

/// The same text with every symbol this product uses written in ASCII.
///
/// The marks are chosen to stay distinct from one another, because they are the
/// whole point: six verdicts that read the same would be worse than six that look
/// plain. Punctuation folds the way a typewriter would have written it.
pub(crate) fn folded(text: &str) -> String {
    text.chars().fold(String::new(), |mut said, character| {
        match character {
            '✓' => said.push('+'),
            '✗' => said.push('x'),
            '·' => said.push('.'),
            '⚠' => said.push('!'),
            '→' => said.push_str("->"),
            '—' => said.push_str("--"),
            '–' | '─' => said.push('-'),
            '…' => said.push_str("..."),
            '“' | '”' => said.push('"'),
            '‘' | '’' => said.push('\''),
            other => said.push(other),
        }
        said
    })
}

/// Print a line to standard output, as this terminal can render it.
pub(crate) fn said(line: &str) {
    println!("{}", rendered(line));
}

/// Print a line to standard error, as this terminal can render it.
///
/// A failure goes to standard error so a script can read the answer on standard
/// output and a person can read the problem beside it.
pub(crate) fn complained(line: &str) {
    eprintln!("{}", rendered(line));
}

/// Whether this run's output is read or parsed, settled once at startup.
///
/// A latch for the same reason the locale beside it is one: it is a property of the
/// run rather than of any call. Twenty-six places report a failure, and a flag
/// threaded to all of them is twenty-six chances to be told wrong — while the one
/// that was missed would report a failure as prose to a script that asked for
/// otherwise, which is the case nobody tests.
static FOR_A_PARSER: OnceLock<bool> = OnceLock::new();

/// Settle who this run's output is for, and say what is in force.
///
/// The first call decides, and a later one is told what the first settled, so what
/// comes back is what is actually in force rather than what this caller asked for.
pub(crate) fn settle_audience(parsed: bool) -> bool {
    *FOR_A_PARSER.get_or_init(|| parsed)
}

/// Whether what this run puts out will be parsed rather than read.
pub(crate) fn for_a_parser() -> bool {
    *FOR_A_PARSER.get().unwrap_or(&false)
}

/// Put a refusal out exactly as it is, for something that will parse it.
///
/// The error stream's half of the parser's door. A refusal is output like any
/// other, and a script that asked for something it could parse asked about the
/// failures too — they are the answers it most needs to act on.
pub(crate) fn refused(line: &str) {
    eprintln!("{line}");
}

/// Put a line out exactly as it is, for something that will parse it.
///
/// The other door of the same funnel, and the reason it exists is that folding is
/// wrong here. Folding decides what a person's terminal can draw, and there is no
/// person on the other end of `--json`. It is not merely unnecessary but damaging:
/// the fold writes a curly quote as `"`, and inside a JSON string that is not a
/// character but the end of it, so a release name containing one arrives as
/// something that will not parse. `--json` is for scripts, and a script is exactly
/// where `LC_ALL=C` is set.
pub(crate) fn emitted(line: &str) {
    println!("{line}");
}

/// Put a question out and leave the cursor beside it, for an answer on the same line.
///
/// No newline, because the answer is typed where the cursor is left. Flushed for the
/// same reason: standard output is buffered when it is a terminal only up to a
/// newline, and a question without one would sit in the buffer while the program
/// waited to be answered — the operator staring at nothing, the program at them.
pub(crate) fn asked(line: &str) {
    print!("{}", rendered(line));
    let _ = std::io::stdout().flush();
}

/// Print a line, as this terminal can render it.
///
/// Takes what `println!` takes, so a call site changes by one word rather than
/// being rewritten — which is what made converting thirty of them worth doing.
macro_rules! say {
    () => { $crate::say::said("") };
    ($($arg:tt)*) => { $crate::say::said(&format!($($arg)*)) };
}

/// Print a line exactly as it is, for something that will parse it.
macro_rules! emit {
    ($($arg:tt)*) => { $crate::say::emitted(&format!($($arg)*)) };
}

/// Print a line to standard error, as this terminal can render it.
macro_rules! complain {
    () => { $crate::say::complained("") };
    ($($arg:tt)*) => { $crate::say::complained(&format!($($arg)*)) };
}

pub(crate) use {complain, emit, say};

#[cfg(test)]
mod tests {
    use super::{
        asked, complained, emitted, folded, for_a_parser, refused, rendered, said, settle,
        settle_audience, shown, unicode,
    };

    /// Everything printed for a person is made plain here, and not by whoever asked.
    ///
    /// Two callers had no report to build and so passed nothing through `Lines::put`:
    /// Compose's own stdout, printed line by line as an image pulls or a stack starts,
    /// and the walkthrough's live narration, whose detail is a catalogue's title. Both
    /// are somebody else's text, and `\x1b[2J` in either clears the operator's terminal.
    #[test]
    fn somebody_elses_text_is_made_plain_on_its_way_out() {
        // Compose's own line, as `engine::emit_line` indents it.
        let composed = rendered("  sonarr Pulling fs layer\u{1b}[2J");
        assert!(!composed.contains('\u{1b}'), "{composed:?}");
        assert!(composed.contains("Pulling fs layer"), "{composed:?}");
        // And the shapes a terminal obeys without being a control character at all.
        for hidden in ['\u{202e}', '\u{200b}', '\u{feff}'] {
            let name = rendered(&format!("  Some{hidden}Release"));
            assert_eq!(name, "  SomeRelease", "U+{:04X}", u32::from(hidden));
        }
    }

    /// The breaks a caller writes into one call are ours and survive.
    ///
    /// A line feed is the first thing a plain-text rule takes, and half the questions
    /// setup asks are written with a blank line in front of them. Made plain line by
    /// line, so the spacing somebody wrote stays and the escape somebody else sent goes.
    #[test]
    fn the_blank_line_a_caller_asked_for_is_still_there() {
        assert_eq!(
            rendered("\nWhat would you like to do?"),
            "\nWhat would you like to do?"
        );
        assert_eq!(rendered("first\n\nthird"), "first\n\nthird");
        assert_eq!(rendered("trailing\n"), "trailing\n");
        assert_eq!(rendered("a\u{1b}[2Jb\nc\rd"), "a[2Jb\ncd");
    }

    /// A locale that names a charset has told us what it can do; one that is unset
    /// has told us nothing, and the requirement asks for a fallback where Unicode
    /// is unsupported rather than wherever it is unproven.
    #[test]
    fn a_locale_is_believed_only_where_it_says_something() {
        for said in ["en_GB.UTF-8", "C.UTF-8", "en_US.utf8", "nl_NL.UTF-8"] {
            assert!(unicode(Some(said)), "{said} names UTF-8");
        }
        for said in ["C", "POSIX", "en_US.ISO-8859-1", "ja_JP.eucJP"] {
            assert!(!unicode(Some(said)), "{said} says it cannot");
        }
        assert!(unicode(None), "nothing said is not an objection");
        assert!(unicode(Some("")), "and neither is an empty answer");
        assert!(
            unicode(Some("en_GB")),
            "a locale naming no charset has not refused"
        );
    }

    /// The marks are the point: six verdicts that folded to the same character
    /// would be worse than six that look plain.
    #[test]
    fn every_mark_folds_to_something_of_its_own() {
        let marks = "✓✗·⚠–";
        let folded: Vec<char> = folded(marks).chars().collect();

        assert_eq!(folded.len(), marks.chars().count(), "one for one");
        let mut seen = folded.clone();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), folded.len(), "and all different: {folded:?}");
    }

    #[test]
    fn punctuation_folds_the_way_a_typewriter_would() {
        assert_eq!(folded("one — two"), "one -- two");
        assert_eq!(folded("→ do this"), "-> do this");
        assert_eq!(folded("wait…"), "wait...");
        assert_eq!(folded("“quoted”"), "\"quoted\"");
        assert_eq!(folded("‘quoted’"), "'quoted'");
        assert!(folded("plain ascii").is_ascii());
    }

    #[test]
    fn text_that_needs_no_folding_is_unchanged() {
        assert_eq!(folded("nothing to do here"), "nothing to do here");
    }

    /// A terminal that can show it gets it as written; one that cannot gets it
    /// folded. Asked of the decision rather than of the latch, so the test settles
    /// nothing for the tests after it.
    #[test]
    fn what_a_terminal_gets_depends_on_what_it_can_show() {
        assert_eq!(shown("kept — as written", false), "kept — as written");
        assert_eq!(shown("folded — as needed", true), "folded -- as needed");
    }

    /// Settling twice is not two answers. A surface that asked second is told what
    /// is in force, because that is what its output will actually be rendered as —
    /// and being told otherwise is how a caller comes to believe a fold happened
    /// that did not.
    ///
    /// The locale here is deliberately the one the first call would have chosen
    /// anyway: this test settles a process-wide latch, and the tests beside it read
    /// the decision directly rather than the latch, so what is latched must be the
    /// ordinary answer rather than one of them.
    #[test]
    fn what_is_settled_first_is_what_every_later_caller_is_told() {
        let first = settle(Some("en_GB.UTF-8"));

        assert!(!first, "a UTF-8 terminal is not folded");
        assert_eq!(
            settle(Some("C")),
            first,
            "the second caller is told what is in force, not what it asked for"
        );
    }

    /// Settled once, like the locale beside it, and for the same reason: it is a
    /// property of the run rather than of any call.
    ///
    /// The value latched here is deliberately the default one. This settles a
    /// process-wide value, and every test beside it expects output for a person —
    /// so what is latched has to be that, or this test would decide for them.
    #[test]
    fn who_the_output_is_for_is_settled_once_too() {
        let first = settle_audience(false);

        assert!(
            !first,
            "output is read by a person unless a run says otherwise"
        );
        assert_eq!(
            settle_audience(true),
            first,
            "the second caller is told what is in force, not what it asked for"
        );
        assert_eq!(for_a_parser(), first, "and asking plainly agrees");
    }

    /// The two ends of the funnel. Asserted only to the extent that they run: what
    /// they put where is the architecture test's to guard, and reading back this
    /// process's own streams would be a harness rather than a test.
    #[test]
    fn both_ends_of_the_funnel_take_a_line() {
        said("an ordinary line — folded or not");
        complained("a line about a failure");
        asked("and a question — answered beside it");
        emitted("{\"and\":\"a document nobody reads\"}");
        refused("{\"nor\":\"this one\"}");
    }
}
