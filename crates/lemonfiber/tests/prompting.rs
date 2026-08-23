//! What stands between a question and the answer to it.
//!
//! Nothing does, and that is the whole of what is checked here. A prompt that
//! expires disadvantages exactly the people the rest of this care is for — anyone
//! who reads slowly, types slowly, or was called away mid-sentence — and where a
//! time limit exists it has to be generous, stated, and something the operator can
//! extend. None exists, so none of that has to be built, and this is what will say
//! so when somebody adds one.
//!
//! Proved by reading the source rather than by running a command, and the reason is
//! worth stating plainly: a question is only ever put where standard input is a
//! terminal, so a test process — whose input is a pipe — cannot reach one to time.
//! Nothing here can watch a prompt not expire. What it can do is watch for the two
//! things a prompt that expired would have to be built out of.

use std::path::Path;

/// Everything that could cut a wait short.
///
/// A blocking read has no deadline of its own, so a prompt that gave up would have
/// to be given one: a clock to measure against, or a second thread to do the
/// waiting while the first watched the time. Either names something here.
const A_CLOCK: [&str; 9] = [
    "Duration", "Instant", "timeout", "deadline", "elapsed", "sleep", "recv", "spawn", "channel",
];

/// The one wire to a person. Standard input is read here and the test below says
/// there is nowhere else, which is what makes the three of these the whole path
/// rather than a sample of it.
const KEYBOARD: &str = "crates/lemonfiber/src/keyboard.rs";

/// What a question is and what an answer means.
const ASKING: &str = "crates/lemonfiber/src/prompt.rs";

/// The wizard's own questions, read as a directory, so a module added beside these
/// is guarded the day it arrives.
const ASKED: &str = "crates/lemonfiber/src/prompt";

/// The one question that is not the wizard's: whether to let downloads finish.
const STOPPING: &str = "crates/lemonfiber/src/stopping.rs";

/// The workspace root, from where this test's crate sits.
fn workspace() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap_or_else(|| Path::new("."))
}

/// Every source file a question travels through, as text.
fn along_the_path() -> Vec<(String, String)> {
    let root = workspace();
    let mut read: Vec<(String, String)> = Vec::new();
    for named in [KEYBOARD, ASKING, STOPPING] {
        if let Ok(text) = std::fs::read_to_string(root.join(named)) {
            read.push((named.to_owned(), text));
        }
    }
    if let Ok(entries) = std::fs::read_dir(root.join(ASKED)) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|kind| kind == "rs") {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    read.push((path.to_string_lossy().into_owned(), text));
                }
            }
        }
    }
    read
}

/// Every source file in the binary, as text.
fn surfaces() -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = Vec::new();
    let mut left = vec![workspace().join("crates/lemonfiber/src")];
    while let Some(here) = left.pop() {
        let Ok(entries) = std::fs::read_dir(&here) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                left.push(path);
            } else if path.extension().is_some_and(|kind| kind == "rs") {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    found.push((path.to_string_lossy().into_owned(), text));
                }
            }
        }
    }
    found
}

/// This binary reads from a person in one place.
///
/// Not a tidiness rule — it is what makes the guard below a statement about every
/// question rather than about the four files somebody remembered to list. A second
/// place to read from would be a second place to put a deadline, and nothing would
/// be watching it.
#[test]
fn every_way_this_binary_reads_from_a_person_is_in_one_file() {
    let mut elsewhere: Vec<String> = Vec::new();
    for (path, text) in surfaces() {
        let where_it_lives = path.replace('\\', "/");
        if where_it_lives.ends_with("src/keyboard.rs") {
            continue;
        }
        for (number, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue;
            }
            if ["read_line", "prompt_password", "stdin()"]
                .iter()
                .any(|reading| trimmed.contains(reading))
            {
                elsewhere.push(format!("{where_it_lives}:{}: {trimmed}", number + 1));
            }
        }
    }

    assert!(
        elsewhere.is_empty(),
        "a second way to read from a person, which nothing is guarding: {elsewhere:?}"
    );
}

/// No question this binary asks is timed.
///
/// Nothing on the path from a question to its answer keeps time. Add one and this
/// fails, which is the moment to answer the rest of the requirement instead: a
/// limit has to be long enough to be no limit for most people, said in the prompt
/// so nobody is surprised by it, and extendable by whoever it caught.
#[test]
fn nothing_between_a_question_and_its_answer_keeps_time() {
    let along = along_the_path();
    assert!(
        along.iter().any(|(path, _)| path.ends_with("keyboard.rs")),
        "the wire to a person was not read, so this guarded nothing"
    );

    let mut timing: Vec<String> = Vec::new();
    for (path, text) in along {
        for (number, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue;
            }
            for kept in A_CLOCK {
                if trimmed.contains(kept) {
                    timing.push(format!("{path}:{}: {trimmed}", number + 1));
                }
            }
        }
    }

    assert!(
        timing.is_empty(),
        "a question that can expire has to be generous, stated and extendable: {timing:?}"
    );
}

/// An answer has nowhere to say that nobody gave one.
///
/// The shape is the second half of the argument, and the sturdier half. Asking
/// hands back the words that were typed and nothing else — no absence, no reason —
/// so a caller cannot decide to stop waiting without changing what asking means for
/// every caller at once. A deadline cannot be added quietly at one call site.
#[test]
fn what_asking_hands_back_cannot_report_that_nobody_answered() {
    let along = along_the_path();
    let asking = match along.iter().find(|(path, _)| path.ends_with("prompt.rs")) {
        Some((_, text)) => text.clone(),
        None => String::new(),
    };

    assert!(
        asking.contains("-> String;"),
        "asking no longer hands back the words themselves: {asking}"
    );
    for expiring in ["-> Option<String>", "-> Result<String"] {
        assert!(
            !asking.contains(expiring),
            "asking can now say that nobody answered, which is the shape a deadline \
             takes — it has to be generous, stated and extendable"
        );
    }
}
