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

/// What was settled, for a surface that must pick a shape rather than a character.
///
/// The fold rewrites a line after it is built, which is enough for everything that
/// differs by a mark. A drawing does not work that way: two half-height blocks are
/// one row of cells and the ASCII standing in for them is two, so the choice is made
/// before there is a line to fold rather than after.
///
/// Reads rather than settles. Asking a question here must not decide the answer for
/// the run — that is the edge's to make, once, and a reader that latched would let
/// whichever surface happened to draw first quietly overrule it.
pub(crate) fn folding() -> bool {
    *ASCII_ONLY.get().unwrap_or(&false)
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

/// Put a refusal out for something that will parse it.
///
/// The error stream's half of the parser's door. A refusal is output like any
/// other, and a script that asked for something it could parse asked about the
/// failures too — they are the answers it most needs to act on.
pub(crate) fn refused(line: &str) {
    eprintln!("{}", written(line));
}

/// Put a line out for something that will parse it.
///
/// The other door of the same funnel, and the reason it exists is that folding is
/// wrong here. Folding decides what a person's terminal can draw, and there is no
/// person on the other end of `--json`. It is not merely unnecessary but damaging:
/// the fold writes a curly quote as `"`, and inside a JSON string that is not a
/// character but the end of it, so a release name containing one arrives as
/// something that will not parse. `--json` is for scripts, and a script is exactly
/// where `LC_ALL=C` is set.
pub(crate) fn emitted(line: &str) {
    println!("{}", written(line));
}

/// The document as it goes out, with nothing in it a terminal would obey.
///
/// Both halves of the parser's door pass through here, for the reason both halves
/// of the person's door pass through [`rendered`]: a rule remembered at each call
/// site is a rule forgotten at one.
///
/// Making it plain is what the other door does and is the wrong answer here — a
/// script asked for a value it could match against what the service holds, and a
/// name with a character taken out of it no longer does. Written out as a `\uXXXX`
/// it is the same string to anything that reads JSON, and six ASCII characters to
/// the terminal the document is being printed to.
///
/// This door had nothing at all, on the stated grounds that serialising had already
/// made the text safe. It has, below a space; above it `serde_json` carries the
/// character raw, so a release title holding a right-to-left override reached a
/// terminal through `--json` while every other way out disarmed it.
fn written(line: &str) -> String {
    lemonfiber_core::text::escaped(line)
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
mod tests;
