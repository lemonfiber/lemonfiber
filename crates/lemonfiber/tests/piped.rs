//! What a pipe gets.
//!
//! Asked of the command rather than of a function. The question is what lands in a
//! file when somebody redirects this, and a renderer handing back a string cannot
//! answer it: whether a stream is a terminal is decided by the process, and only a
//! real process with a real pipe on its output has an answer. Cargo builds the
//! binary for this target and names it, so what runs here is what ships.
//!
//! Every run is given a machine of its own and an empty environment. A locale
//! decides how marks are rendered, a colour preference decides whether the viewer
//! paints, and a setting decides what several of these commands say — a run that
//! inherited any of them would pass or fail by whose machine it was on.

use std::path::{Path, PathBuf};

/// The binary Cargo built for this test.
const BINARY: &str = env!("CARGO_BIN_EXE_lemonfiber");

/// The commands a pipe can be given with no engine, no network and no stack.
///
/// Six rather than one, because the ways out differ: `--help` is written by the
/// argument parser and is the one place a library could colour on its own, a
/// report and a list and a glossary entry go out as prose, a bare run is the
/// greeting, and a name that is not a category is a refusal on the error stream.
const UNATTENDED: [&[&str]; 6] = [
    &["--help"],
    &["version"],
    &[],
    &["explain", "hardlink"],
    &["forms"],
    &["doctor", "--only", "nonsense"],
];

/// A machine of its own for one test, with nothing on it yet.
fn machine(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("lemonfiber-piped-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::create_dir_all(root.join("config"));
    let _ = std::fs::create_dir_all(root.join("data"));
    root
}

/// Run the command on that machine with both streams on a pipe, and hand back what
/// each of them received.
///
/// The search path is emptied along with the rest: with no engine to find, what
/// these commands say is what they say anywhere, and the one that asks about a
/// daemon reports the same absence every time.
fn piped(home: &Path, argv: &[&str]) -> Result<(String, String), String> {
    let ran = std::process::Command::new(BINARY)
        .args(argv)
        .env_clear()
        .env("PATH", "")
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_DATA_HOME", home.join("data"))
        .current_dir(home)
        .output()
        .map_err(|err| err.to_string())?;
    Ok((
        String::from_utf8_lossy(&ran.stdout).into_owned(),
        String::from_utf8_lossy(&ran.stderr).into_owned(),
    ))
}

/// Every character in this text that a terminal would take as an instruction.
///
/// A tab and a newline are layout, and everything else below a space is not: the
/// escape that starts a sequence, the carriage return that writes over the line
/// just printed, the bell. The single-code-point form of an escape and the rest of
/// the upper control block are here too, being the same instructions written
/// shorter.
fn obeyed(text: &str) -> Vec<String> {
    text.chars()
        .filter(|character| {
            matches!(character, '\u{0}'..='\u{8}' | '\u{b}'..='\u{1f}' | '\u{7f}'..='\u{9f}')
        })
        .map(|character| format!("U+{:04X}", u32::from(character)))
        .collect()
}

/// The lines of this text with the empty ones dropped.
fn spoken(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .collect()
}

/// Every line in this text that is the line before it said again.
fn written_twice(text: &str) -> Vec<String> {
    let mut again: Vec<String> = Vec::new();
    for pair in spoken(text).windows(2) {
        if let [before, then] = pair {
            if before == then {
                again.push((*before).to_owned());
            }
        }
    }
    again
}

/// Nothing a pipe receives can drive the terminal it may end up on.
///
/// A log file full of escapes is unreadable, and the escapes are worse than noise:
/// a file replayed with `cat` hands every one of them to the emulator, so a
/// diagnosis somebody saved to send on can clear their screen when they open it.
///
/// Both streams, because a redirect usually takes one and a diagnosis usually
/// arrives on the other.
#[test]
fn nothing_a_pipe_receives_can_drive_the_terminal() {
    let home = machine("no-instructions");

    let mut carried: Vec<String> = Vec::new();
    for argv in UNATTENDED {
        match piped(&home, argv) {
            Ok((out, err)) => {
                for found in obeyed(&out) {
                    carried.push(format!("{argv:?} said {found} on its output"));
                }
                for found in obeyed(&err) {
                    carried.push(format!("{argv:?} said {found} on its error stream"));
                }
            }
            Err(why) => carried.push(format!("{argv:?} would not run: {why}")),
        }
    }

    assert!(
        carried.is_empty(),
        "a pipe was handed instructions for a terminal: {carried:?}"
    );
}

/// No line a pipe receives is the line before it said again.
///
/// The failure this names is a progress bar in a log file. Redrawn in place it is a
/// carriage return before every version of the same line, which the guard above
/// catches; written out plainly it is the same sentence four thousand times, which
/// is this one. Both turn a file somebody has to read into a file they have to
/// search.
#[test]
fn no_line_a_pipe_receives_repeats_the_one_before_it() {
    let home = machine("no-repeats");

    let mut repeated: Vec<String> = Vec::new();
    for argv in UNATTENDED {
        match piped(&home, argv) {
            Ok((out, err)) => {
                for said in written_twice(&out).into_iter().chain(written_twice(&err)) {
                    repeated.push(format!("{argv:?} said {said:?} twice over"));
                }
            }
            Err(why) => repeated.push(format!("{argv:?} would not run: {why}")),
        }
    }

    assert!(
        repeated.is_empty(),
        "a line was written and then written again: {repeated:?}"
    );
}

/// A run nobody is watching says where to go instead of taking the screen.
///
/// This is the one path that would otherwise have the terminal: a bare invocation
/// on a machine already set up opens the dashboard, which holds the screen until
/// somebody leaves it and draws in the alternate buffer to do it. In a pipe, a cron
/// line or a build step there is nobody to leave it, and what would land in the
/// file is a full-screen redraw that never ends.
///
/// So the setting is written first — that is the whole of what "already set up"
/// means here — and then the same bare invocation is made with its output on a
/// pipe. What comes back is four lines of guidance and not one instruction.
#[test]
fn a_run_nobody_is_watching_says_where_to_go_rather_than_taking_the_screen() {
    let home = machine("configured");

    let settled = piped(&home, &["config", "set", "explanations", "on"]);
    assert!(
        settled.is_ok(),
        "the machine could not be set up: {settled:?}"
    );

    // The reason it could not be run stands in for what it said, so a failure to
    // spawn fails the assertion below carrying its own explanation.
    let said = match piped(&home, &[]) {
        Ok((out, _)) => out,
        Err(why) => why,
    };
    let instructions = obeyed(&said);

    assert!(
        said.contains("already set up on this machine"),
        "a bare run in a pipe did not give the guidance: {said:?}"
    );
    assert!(
        instructions.is_empty(),
        "it took the screen instead: {instructions:?}"
    );
}
