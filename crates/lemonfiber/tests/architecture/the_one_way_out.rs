//! Everything this binary says, and the single door it says it through.
//!
//! Four rules about one seam, which is why they are one file: output leaves through
//! `say.rs` and nowhere else, what leaves is treated on the way out, a failure
//! leaves by the stream a failure belongs on, and what a parser reads is never
//! dressed for a person. Each is a hole in the same floor — a bare print, an
//! untreated string, a complaint on stdout, a folded quote inside JSON — and each
//! one still compiles, still reads correctly in review, and is invisible until it
//! reaches the terminal or the script it breaks.
//!
//! Read from the syntax tree rather than by running anything. What is pinned here is
//! which door a line goes through and which stream it lands on; a test that called
//! the reporter would have to capture this process's own output to find out, which
//! is a harness rather than a test.

use std::path::PathBuf;

use crate::shape::{function, Reached};
use crate::source_tree::parsed;

/// The ways to put a line where somebody can read it.
const DOORS: [&str; 4] = ["println", "eprintln", "print", "eprint"];

/// The one file of this crate at `path`, parsed.
fn file(path: &str) -> syn::File {
    let Some((_, file)) = parsed(path).into_iter().next() else {
        unreachable!("{path} is part of this crate");
    };
    file
}

/// Every shipped source file of the workspace, parsed: what is under a `src`
/// directory and is not a test.
fn shipped() -> Vec<(PathBuf, syn::File)> {
    parsed("crates")
        .into_iter()
        .filter(|(path, _)| path.components().any(|part| part.as_os_str() == "src"))
        .filter(|(path, _)| {
            !path.ends_with("tests.rs")
                && !path.components().any(|part| part.as_os_str() == "tests")
        })
        .collect()
}

/// What a parser reads is never rendered for a person.
///
/// Folding decides what a person's terminal can draw, and `--json` has no person on
/// the other end of it. There it is not merely unnecessary but damaging: the fold
/// writes a curly quote as `"`, and inside a JSON string that is not a character but
/// the end of it — so a release name carrying one arrives as something that will not
/// parse at all. `--json` is for scripts, and a script is exactly where `LC_ALL=C`
/// is set, which is what turns folding on. The two meet more often than not.
///
/// Guarded by shape rather than by output, because the fault appears only on a
/// terminal that folds and only for text carrying one of a handful of characters. A
/// test of what was printed would pass on the machine of whoever broke it.
#[test]
fn nothing_a_parser_reads_is_rendered_for_a_person() {
    let rendered: Vec<String> = shipped()
        .iter()
        .filter(|(_, file)| {
            Reached::in_file(file)
                .invoked("say")
                .iter()
                .any(|arguments| arguments.contains("to_json"))
        })
        .map(|(path, _)| path.display().to_string())
        .collect();
    assert!(
        rendered.is_empty(),
        "serialised output put out through the door that folds — use `emit!`: \
         {rendered:?}"
    );

    // And the doors themselves: what a parser reads goes out exactly as it was
    // built, on either stream. A failure is output too, and a script that asked for
    // something it could parse asked about those most of all.
    let say = file("crates/lemonfiber/src/say.rs");
    for door in ["emitted", "refused"] {
        let Some(body) = function(&say, door) else {
            unreachable!("`say.rs` has the door `{door}`");
        };
        let reached = Reached::in_block(&body);
        assert!(
            !reached.calls("rendered") && !reached.calls("folded"),
            "{door} does not fold"
        );
    }
}

/// Output leaves through one place, and this is what keeps it that way.
///
/// The funnel was worth building because a question about *how* something is shown
/// had a hundred and six answers, or in practice none. Nothing about its shape stops
/// the hundred and seventh: a bare print added later still compiles, still reads
/// correctly in review, and silently opts that one line out of every rendering
/// decision the funnel makes. The failure it produces is the cruel kind — the line
/// that skipped the fold would be the one still carrying a tick on the terminal that
/// cannot draw one, which is precisely the terminal the fold exists for.
///
/// Two files are allowed to reach a stream directly, for opposite reasons.
/// `src/say.rs` **is** the funnel. `build.rs` is not talking to a person at all: its
/// output is a protocol Cargo parses, and rendering a directive for a human terminal
/// would corrupt it. Everything under a `src/` directory that is not the funnel goes
/// through the funnel.
#[test]
fn nothing_reaches_a_terminal_except_through_the_one_way_out() {
    let leaks: Vec<String> = shipped()
        .iter()
        .filter(|(path, _)| !path.ends_with("src/say.rs"))
        .filter(|(_, file)| {
            let reached = Reached::in_file(file);
            DOORS.iter().any(|door| !reached.invoked(door).is_empty())
                || reached.calls("prompt_password") && !reached.invoked("format").is_empty()
        })
        .map(|(path, _)| path.display().to_string())
        .collect();
    assert!(
        leaks.is_empty(),
        "these reach a terminal without passing the funnel, so nothing decides how \
         they are rendered: {leaks:?}"
    );
}

/// Every door of the funnel treats the text before it puts it out.
///
/// The funnel exists so that a question about how something is shown has one
/// answer. It has two doors, and for a long time only one of them treated what it
/// was given: the person's half made the text plain, and the parser's half printed
/// it exactly as it arrived — on the stated grounds that serialising had already
/// escaped every control character. It escapes the ones below a space, and carries
/// the C1 controls, the line separators, the bidirectional overrides and the
/// zero-widths raw. So `--json` was the one way out a release title could still
/// reach a terminal through intact, and the sentence saying otherwise was the whole
/// reason nobody looked.
///
/// Pinned by shape rather than by output, for the same reason the funnel's other
/// rules are: what this catches is a **third** door added later, printing directly
/// because that is what the two beside it appear to do. Reading the streams back
/// would be a harness, and would say nothing about the door nobody has written yet.
#[test]
fn nothing_leaves_this_binary_without_passing_through_a_treatment() {
    let reached = Reached::in_file(&file("crates/lemonfiber/src/say.rs"));
    let printed: Vec<&str> = DOORS
        .iter()
        .flat_map(|door| reached.invoked(door))
        .collect();
    let untreated: Vec<&&str> = printed
        .iter()
        .filter(|arguments| !arguments.contains("rendered") && !arguments.contains("written"))
        .collect();

    // What this is reading, asserted before what it found. A funnel that has been
    // renamed or moved leaves nothing to match, and a guard that found no doors
    // would pass while watching an empty file.
    assert!(
        printed.len() >= 3,
        "fewer doors than there are ways to print, so this is reading the wrong \
         file: {} found",
        printed.len()
    );
    assert!(
        untreated.is_empty(),
        "these put text out with neither treatment, so somebody else's control \
         characters reach a terminal through them: {untreated:?}"
    );

    // And the treatments themselves, since a door naming one that does nothing
    // would satisfy the check above without disarming anything.
    let text = file("crates/lemonfiber-core/src/text.rs");
    assert!(
        function(&text, "plain").is_some() && function(&text, "escaped").is_some(),
        "the two treatments are where the doors reach for them"
    );
}

/// A failure reaches the operator on stderr, so a script can read the answer on
/// stdout and a person can read the problem beside it.
///
/// Guarded rather than merely true: `complain` is one function, and the day
/// somebody adds a line to it the difference between `println!` and `eprintln!`
/// is one character and no test. A machine-readable run that mixed its diagnosis
/// into its output would be broken in a way nothing else here would catch.
#[test]
fn a_failure_is_reported_on_stderr_and_never_on_stdout() {
    let exit = file("crates/lemonfiber/src/exit/reporting.rs");
    let (Some(complain), Some(built)) = (function(&exit, "complain"), function(&exit, "reported"))
    else {
        unreachable!("the reporter and what it builds are in `exit/reporting.rs`");
    };
    let complaining = Reached::in_block(&complain);
    assert!(complaining.calls("eprint"), "it reports something at all");
    assert!(
        !complaining.calls("print"),
        "a diagnosis on stdout would corrupt a machine-readable run"
    );

    // Output leaves through one place, so this is pinned where it is decided rather
    // than at each call site: `complain!` is the stderr half of that funnel, and
    // `say!` the stdout half. A reporter reaching for the wrong one is one word.
    let say = file("crates/lemonfiber/src/say.rs");
    let (Some(complained), Some(said)) = (function(&say, "complained"), function(&say, "said"))
    else {
        unreachable!("the funnel's two halves are in `say.rs`");
    };
    assert!(
        !Reached::in_block(&complained)
            .invoked("eprintln")
            .is_empty(),
        "the funnel's error half writes to stderr"
    );
    assert!(
        !Reached::in_block(&said).invoked("println").is_empty(),
        "and its ordinary half writes to stdout"
    );

    for reporter in [&complain, &built] {
        let reached = Reached::in_block(reporter);
        assert!(
            reached.invoked("println").is_empty() && reached.invoked("say").is_empty(),
            "the reporter writes to stdout"
        );
        // Nothing in a failure path may wait for a person: a non-interactive run
        // has nobody to answer, and a prompt there hangs a script for ever.
        assert!(
            !reached.calls("read_line") && !reached.calls("stdin"),
            "the reporter prompts"
        );
    }
}
