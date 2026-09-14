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
//! Read from source text rather than by running anything. What is pinned here is
//! which door a line goes through and which stream it lands on; a test that called
//! the reporter would have to capture this process's own output to find out, which
//! is a harness rather than a test.

use std::fs;

mod source_tree;

use source_tree::{production, sources};

/// The body of a named function in a piece of source, or nothing where it is absent.
///
/// Read from the source rather than by running it, because what this pins is which
/// stream a failure reaches, and a test that called the reporter would have to
/// capture this process's own stderr to find out — a harness rather than a test.
fn body_of<'a>(source: &'a str, signature: &str) -> &'a str {
    source
        .split_once(signature)
        .and_then(|(_, rest)| rest.split_once("\n}\n"))
        .map(|(body, _)| body)
        .unwrap_or_default()
}
/// The text of a call, from its opening bracket to the one that closes it.
///
/// Read by counting brackets rather than by taking a fixed number of lines, because
/// a fixed window is wrong in both directions: it misses a call that formatting has
/// spread wider than the window, and it blames a call for a line that merely follows
/// it. Neither is hypothetical here — `engine.rs` already has an ordinary `say!`
/// sitting seven lines above an unrelated `to_json()`, which a six-line window
/// cleared by one line.
fn invocation(text: &str, opens: usize) -> &str {
    let Some(rest) = text.get(opens..) else {
        return "";
    };
    let mut depth = 0_usize;
    for (at, character) in rest.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return rest.get(..=at).unwrap_or(rest);
                }
            }
            _ => {}
        }
    }
    rest
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
    let mut rendered: Vec<String> = Vec::new();
    for (path, text) in sources() {
        let where_it_lives = path.to_string_lossy().replace('\\', "/");
        if !where_it_lives.contains("/src/") {
            continue;
        }
        let shipped = production(&text);
        for (at, _) in shipped.match_indices("say!(") {
            if !invocation(shipped, at + "say!".len()).contains("to_json()") {
                continue;
            }
            let number = shipped
                .get(..at)
                .map_or(0, |before| before.matches('\n').count() + 1);
            rendered.push(format!("{where_it_lives}:{number}"));
        }
    }
    assert!(
        rendered.is_empty(),
        "serialised output put out through the door that folds — use `emit!`: \
         {rendered:?}"
    );

    // And the doors themselves: what a parser reads goes out exactly as it was
    // built, on either stream. A failure is output too, and a script that asked for
    // something it could parse asked about those most of all.
    let say = std::fs::read_to_string("src/say.rs").unwrap_or_default();
    for door in ["pub(crate) fn emitted", "pub(crate) fn refused"] {
        let body = body_of(&say, door);
        assert!(!body.is_empty(), "{door} was found");
        assert!(
            !body.contains("rendered") && !body.contains("folded"),
            "{door} does not fold: {body}"
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
    let mut leaks: Vec<String> = Vec::new();
    for (path, text) in sources() {
        let where_it_lives = path.to_string_lossy().replace('\\', "/");
        // `build.rs` sits beside `src/` rather than inside it, so naming the funnel
        // is the whole of the exception list.
        if !where_it_lives.contains("/src/") || where_it_lives.ends_with("src/say.rs") {
            continue;
        }
        for (number, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue;
            }
            // One entry per line rather than one per way out: `eprintln!(` contains
            // `println!(`, and a line reported twice reads as two faults.
            //
            // `prompt_password` is here because it is not a macro and was the second
            // leak found: the password crate writes the prompt itself, so the text
            // has to arrive already folded rather than being handed over raw.
            if [
                "println!(",
                "eprintln!(",
                "print!(",
                "eprint!(",
                "prompt_password(format!",
            ]
            .iter()
            .any(|reaching| trimmed.contains(reaching))
            {
                leaks.push(format!("{where_it_lives}:{}: {trimmed}", number + 1));
            }
        }
    }
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
    /// What puts a line where somebody can read it.
    const DOORS: [&str; 3] = ["println!", "eprintln!", "print!"];
    /// The two answers to what happens to it first: for a person, for a parser.
    const TREATMENTS: [&str; 2] = ["rendered(", "written("];

    let say = fs::read_to_string("src/say.rs").unwrap_or_default();
    let shipped = production(&say);
    let mut untreated: Vec<&str> = Vec::new();
    let mut doors = 0_usize;
    for line in shipped.lines() {
        let statement = line.trim_start();
        if !DOORS.iter().any(|door| statement.starts_with(door)) {
            continue;
        }
        doors += 1;
        if !TREATMENTS.iter().any(|how| statement.contains(how)) {
            untreated.push(statement);
        }
    }

    // What this is reading, asserted before what it found. A funnel that has been
    // renamed or moved leaves the loop above matching nothing, and a guard that
    // found no doors would pass while watching an empty file — which is the shape
    // this repository has been caught by twice.
    assert!(
        doors >= DOORS.len(),
        "fewer doors than there are ways to print, so this is reading the wrong \
         file: {doors} found"
    );
    assert!(
        untreated.is_empty(),
        "these put text out with neither treatment, so somebody else's control \
         characters reach a terminal through them: {untreated:?}"
    );

    // And the treatments themselves, since a door naming one that does nothing
    // would satisfy the loop above without disarming anything.
    let core = fs::read_to_string("../lemonfiber-core/src/text.rs").unwrap_or_default();
    assert!(
        core.contains("pub fn plain") && core.contains("pub fn escaped"),
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
    let exit = std::fs::read_to_string("src/exit/reporting.rs").unwrap_or_default();
    let complain = body_of(&exit, "pub(crate) fn complain");
    // The lines themselves are built beside it rather than inside it, so the per-line
    // checks below take both. A guard that kept reading only the two-line caller would
    // pass on anything.
    let built = body_of(&exit, "\npub(crate) fn reported");
    let reporter = format!("{complain}\n{built}");

    assert!(!complain.is_empty(), "the reporter was found");
    assert!(!built.is_empty(), "and what it builds");
    assert!(
        complain.contains(".eprint()"),
        "it reports something at all"
    );
    assert!(
        !complain.contains(".print()"),
        "a diagnosis on stdout would corrupt a machine-readable run"
    );

    // Output leaves through one place now, so this is pinned where it is decided
    // rather than at each of a hundred call sites: `complain!` is the stderr half
    // of that funnel, and `say!` the stdout half. A reporter reaching for the wrong
    // one is the failure this guards, and it is still one word.
    let say = std::fs::read_to_string("src/say.rs").unwrap_or_default();
    assert!(
        say.contains("fn complained") && say.contains("eprintln!"),
        "the funnel's error half writes to stderr"
    );
    assert!(
        say.contains("fn said") && say.contains("println!"),
        "and its ordinary half writes to stdout"
    );

    for line in reporter.lines() {
        let statement = line.trim_start();
        assert!(
            !statement.starts_with("println!") && !statement.starts_with("say!"),
            "reports on stdout: {statement}"
        );
        // Nothing in a failure path may wait for a person: a non-interactive run
        // has nobody to answer, and a prompt there hangs a script for ever.
        assert!(
            !statement.contains("read_line") && !statement.contains("stdin"),
            "prompts while reporting: {statement}"
        );
    }
}
