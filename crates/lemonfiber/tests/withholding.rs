//! Which settings are displayed in full, over every namespace that reaches the file.
//!
//! Two corpora, because there are two writers. The stack declares its settings in
//! the embedded `.env.example` — read from the embedded copy rather than from the
//! checked-out submodule, for the same reason the published ports are: those are
//! the bytes a machine ends up running, and the directory beside them belongs to
//! another repository that a clone need not have populated at all.
//!
//! lemonfiber declares its own in [`lemonfiber_core::config::SETTINGS`], and until
//! that list existed this guard could not see them. That was the whole namespace
//! setup collects an indexer key and a Usenet password into — so the guard's claim,
//! that a new credential is red until somebody argues for it, was false for exactly
//! the settings it most needed to be true for. Three of those keys happened to
//! carry a marker word, which is luck and not a check.
//!
//! What runs here is the display path itself rather than the name test under it:
//! the claim is that an operator's value does not come back out, which is a
//! property of what is rendered and not of what a predicate says about a name.
//!
//! The second guard below is about the same path from the other end. A value is
//! withheld where the settings are read, so a surface that read them for itself
//! would be outside all of this — and the dashboard now has a screen that shows
//! them.

use std::collections::BTreeSet;

use lemonfiber::cli::STACK;
use lemonfiber_core::config::display::SHOWN;
use lemonfiber_core::config::env::EnvFile;
use lemonfiber_core::config::store::showing;
use lemonfiber_core::config::SETTINGS;

mod source_tree;

/// The file the stack declares its settings in.
const SETTINGS_FILE: &str = ".env.example";

/// The settings the embedded stack declares.
///
/// Read through the same parser the binary reads an operator's own file with, so
/// that what counts as a declared setting is decided once. A second reader would
/// answer this question about a slightly different set of lines than the one the
/// configuration surface serves, and the two would drift without either being
/// wrong.
fn declared() -> EnvFile {
    let text = STACK
        .get_file(SETTINGS_FILE)
        .and_then(include_dir::File::contents_utf8)
        .unwrap_or_default();
    let settings = EnvFile::parse(text);
    assert!(
        !settings.keys().is_empty(),
        "the embedded stack declares no settings in {SETTINGS_FILE}, which means this is \
         reading the wrong thing"
    );
    settings
}

/// Every setting that reaches an operator's file, from either writer.
fn every_setting() -> BTreeSet<String> {
    let stack = declared();
    let mut names: BTreeSet<String> = stack.keys().into_iter().map(str::to_owned).collect();
    names.extend(SETTINGS.iter().map(|name| (*name).to_owned()));
    assert!(
        names.len() > stack.keys().len(),
        "lemonfiber's own settings are all in the stack's file, which means SETTINGS is \
         not being read"
    );
    names
}

/// A stand-in for whatever an operator has actually put in a setting.
///
/// Every credential in the shipped file is empty, waiting to be filled in, and an
/// empty value is displayed as itself. The question is what happens once one is
/// filled in, so one is.
fn a_value() -> String {
    "x".repeat(24)
}

/// The settings whose value comes back out of the display path as it was written.
fn displayed_in_full(names: &BTreeSet<String>) -> Vec<&str> {
    let supplied = a_value();
    names
        .iter()
        .map(String::as_str)
        .filter(|name| showing(name, &supplied).value == supplied)
        .collect()
}

/// Nothing is displayed with its value without a reason written down.
///
/// The default is the safe one and the exception is the thing that costs somebody
/// a sentence, which is the only arrangement that survives a setting being added
/// by whoever is in a hurry. A new setting holding a credential does not need to
/// be noticed in review: it is red until somebody argues for it.
///
/// Structural now rather than emergent, which is the point of the list: the display
/// path consults it, so this can only fail if something learns to display a value
/// without asking. That is worth a test — it is the bypass nobody would notice.
#[test]
fn no_setting_is_displayed_without_a_declared_reason() {
    let listed: BTreeSet<&str> = SHOWN.iter().map(|(name, _)| *name).collect();
    let known = every_setting();
    let undecided: Vec<&str> = displayed_in_full(&known)
        .into_iter()
        .filter(|name| !listed.contains(name))
        .collect();
    assert!(
        undecided.is_empty(),
        "these are served with their values to anything that reads the configuration, a \
         browser included, and nothing says why: {undecided:?}"
    );
}

/// A setting nobody has decided about keeps its value to itself.
///
/// The claim the allow-list is bought for, and the one a marker list cannot make.
/// None of these carries a word `is_secret` recognises, every one of them is a
/// credential in some stack, and the last is the case that matters most: the
/// setting that leaks is the one nobody had thought of when the rule was written.
#[test]
fn a_setting_nobody_has_argued_for_is_withheld() {
    let supplied = a_value();
    let served: Vec<&str> = [
        "OPENVPN_USER",
        "PLEX_CLAIM",
        "DISCORD_WEBHOOK",
        "DB_PWD",
        "SESSION_SALT",
        "DATABASE_URL",
        "SOME_SERVICE_ADDED_NEXT_YEAR",
    ]
    .into_iter()
    .filter(|name| showing(name, &supplied).value == supplied)
    .collect();
    assert!(
        served.is_empty(),
        "these reach a browser with their values and no rule recognises their names: \
         {served:?}"
    );
}

/// Every credential lemonfiber's own setup collects stays out of the display path.
///
/// Named one at a time rather than derived, because deriving them from the same
/// list the display path reads would be the check agreeing with itself. These are
/// the four values an operator hands over or lemonfiber mints, and the outcome for
/// each of them is asserted directly.
#[test]
fn no_credential_lemonfiber_writes_is_ever_displayed() {
    let supplied = a_value();
    for name in [
        "INDEXER_APIKEY",
        "USENET_PASS",
        "USENET_USER",
        "QBITTORRENT_PASSWORD",
        "JELLYFIN_ADMIN_PASSWORD",
    ] {
        let seen = showing(name, &supplied);
        assert!(seen.secret, "{name} is not marked withheld");
        assert!(!seen.value.contains(&supplied), "{name} -> {}", seen.value);
    }
}

/// Nothing stays on the list after no writer produces it any more.
///
/// The other direction of the same rule, and it is not the same test: the one above
/// starts from what is declared, so an entry for a setting that has been dropped
/// would pass it in silence. An exception that no longer excepts anything reads as a
/// decision somebody made about the stack as it is, and it is a decision about a
/// stack that is gone.
#[test]
fn nothing_stays_on_the_list_after_no_writer_produces_it() {
    let known = every_setting();
    let stale: Vec<&str> = SHOWN
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !known.contains(*name))
        .collect();
    assert!(
        stale.is_empty(),
        "these are written down as deliberately displayed and no writer produces them — a \
         setting the stack dropped, or one lemonfiber stopped naming: {stale:?}"
    );
}

/// Every displayed setting says why it is one.
///
/// An exception list whose entries may be blank is a list of names, and a name
/// explains nothing to whoever reads it next. A reason is a sentence somebody can
/// disagree with.
#[test]
fn every_displayed_setting_says_why_it_is_one() {
    let silent: Vec<&str> = SHOWN
        .iter()
        .filter(|(_, reason)| reason.split_whitespace().count() < 4)
        .map(|(name, _)| *name)
        .collect();
    assert!(
        silent.is_empty(),
        "these are displayed with their values and the list does not say what makes that \
         safe: {silent:?}"
    );
}

/// The module trees that draw a full screen, which is where a setting could reach
/// a terminal without passing the display path.
///
/// Trees rather than files. `terminal.rs` was named here as a file and stopped being
/// the whole of the terminal the day `terminal/` grew beside it: `dashboard.rs` and
/// `screen.rs` moved under that directory, and neither `/src/terminal.rs` nor
/// `/src/dashboard` matches `/src/terminal/dashboard.rs` — so the file that draws the
/// dashboard sat outside this guard while the guard went on passing. A name that
/// matches a directory as readily as a file is what stops that happening again, and
/// the test below refuses to let one of these match nothing at all.
const DRAWING: [&str; 4] = ["acting", "dashboard", "pane.rs", "terminal"];

/// How the settings are read, which no screen may do for itself.
const READING: [&str; 2] = ["config::store", "env_file"];

/// Nothing that draws a screen reads a setting for itself.
///
/// Withholding happens where the settings are read. A screen that opened the
/// environment file would be outside that path, and would put on a terminal exactly
/// the values the list above exists to keep out of a report. The dashboard asks for
/// them by naming the read a browser names, which comes to `config show` and to the
/// display path under it.
#[test]
fn nothing_that_draws_a_screen_reads_a_setting_for_itself() {
    let mut reaching: Vec<String> = Vec::new();
    let mut watched: Vec<&str> = Vec::new();
    for (path, text) in source_tree::sources() {
        let where_it_lives = path.to_string_lossy().replace('\\', "/");
        let Some(tree) = DRAWING
            .iter()
            .find(|tree| where_it_lives.contains(&format!("/src/{tree}")))
        else {
            continue;
        };
        watched.push(tree);
        for (number, line) in source_tree::production(&text).lines().enumerate() {
            if READING.iter().any(|how| line.contains(how)) {
                reaching.push(format!("{where_it_lives}:{}", number + 1));
            }
        }
    }

    // What this guard is actually reading, asserted before what it found. A name here
    // that matches nothing is a tree renamed out from under it, and the failure that
    // causes is silence: the guard goes on passing about code it can no longer see.
    let unwatched: Vec<&&str> = DRAWING
        .iter()
        .filter(|tree| !watched.contains(tree))
        .collect();
    assert!(
        unwatched.is_empty(),
        "these name no source file, so this guard is watching less than it says it \
         is — a screen has been renamed or moved: {unwatched:?}"
    );

    // And the other half of the same question, which the list above had and this one
    // did not. `DRAWING` fails when it names a tree nothing matches; `READING` could
    // name a way of reaching the settings that nothing does any more, and the failure
    // would look identical from outside — a green run about a rule nobody is held to.
    //
    // Read from what ships, and **not** from the tests. The corpus the crawler
    // returns is everything under `crates/`, this file among it — so a first version
    // of this check found each name in the `const` that declares it and passed about
    // nothing at all, which is the shape it was written to catch.
    let shipped: String = source_tree::sources()
        .into_iter()
        .filter(|(path, _)| path.to_string_lossy().replace('\\', "/").contains("/src/"))
        .map(|(_, text)| text)
        .collect();
    let gone: Vec<&&str> = READING
        .iter()
        .filter(|how| !shipped.contains(**how))
        .collect();
    assert!(
        gone.is_empty(),
        "these name no way anything reads the settings, so watching for them holds \
         nothing — the reading has been renamed and this guard did not follow: {gone:?}"
    );

    assert!(
        reaching.is_empty(),
        "a screen reads settings by asking the core for them, never by reading them: \
         {reaching:?}"
    );
}
