//! Which of the settings the stack this binary carries are displayed in full.
//!
//! Read from the embedded copy — the same `STACK` the binary writes to an
//! operator's disk — rather than from the checked-out submodule, for the same
//! reason the published ports are: those are the bytes a machine ends up running,
//! and the directory beside them belongs to another repository that a clone need
//! not have populated at all.
//!
//! Withholding decides by name, so a setting whose name carries a credential and
//! none of the words the withholding recognises is displayed with its value. That
//! answer now reaches a browser rather than only a terminal, and a heuristic
//! nothing reads back is a heuristic that is right until the day it is not.
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
use lemonfiber_core::config::env::EnvFile;
use lemonfiber_core::config::store::showing;

mod source_tree;

/// The file the stack declares its settings in.
const SETTINGS: &str = ".env.example";

/// The settings displayed with their values, and why each one is.
///
/// Everything else the stack declares is withheld. These are the ones whose value
/// is the answer an operator came for — a path, an address, a schedule — and
/// which tell whoever reads them nothing they could sign in with.
///
/// A setting that arrives displayed and is not written down here fails, and the
/// second half of the entry is where somebody has to say what makes displaying it
/// safe. The list is the decision; this is where it is reviewable.
const SHOWN: &[(&str, &str)] = &[
    (
        "DATA_ROOT",
        "the one path an operator has to get right, and the one they check first",
    ),
    (
        "PUID",
        "the user id everything under the data root is owned by",
    ),
    (
        "PGID",
        "the group id everything under the data root is owned by",
    ),
    ("TZ", "schedules and log timestamps are read in this zone"),
    (
        "LAN_BIND",
        "the address the household tier is published on, which an operator narrows by hand",
    ),
    (
        "VPN_PROVIDER",
        "which provider's servers the tunnel dials, and not the account on them",
    ),
    (
        "VPN_COUNTRIES",
        "where the tunnel comes out, which is changed often and checked after",
    ),
    (
        "VPN_PORT_FORWARDING",
        "whether a forwarded port is asked for at all",
    ),
    (
        "QBITTORRENT_USERNAME",
        "an account name says who signs in; the password beside it is what signs in",
    ),
    (
        "UMASK",
        "the file mode extracted downloads land on disk with",
    ),
    (
        "FLARESOLVERR_LOG_LEVEL",
        "how much the challenge solver writes to its own log",
    ),
    (
        "SEERR_LOG_LEVEL",
        "how much the request service writes to its own log",
    ),
    (
        "RECYCLARR_CRON",
        "when the quality sync runs, which explains a profile that has not moved",
    ),
    (
        "HOMEPAGE_ALLOWED_HOSTS",
        "the addresses the dashboard answers for, and a wrong one is why it refuses",
    ),
    (
        "HOMEPAGE_VAR_LAN_HOST",
        "the address the dashboard's household links point at",
    ),
    (
        "HOMEPAGE_VAR_QBITTORRENT_USER",
        "the account name the dashboard widget signs in as, beside a withheld password",
    ),
    (
        "UN_SONARR_0_URL",
        "where the extractor reaches the television service",
    ),
    (
        "UN_RADARR_0_URL",
        "where the extractor reaches the film service",
    ),
    (
        "UN_LIDARR_0_URL",
        "where the extractor reaches the music service",
    ),
    (
        "DOMAIN",
        "the hostname real certificates are obtained for, which is public in the certificate",
    ),
    ("NAS_HOST", "the machine the network mount is exported from"),
    (
        "NAS_EXPORT",
        "the share on that machine the data root lives on",
    ),
];

/// The settings the embedded stack declares.
///
/// Read through the same parser the binary reads an operator's own file with, so
/// that what counts as a declared setting is decided once. A second reader would
/// answer this question about a slightly different set of lines than the one the
/// configuration surface serves, and the two would drift without either being
/// wrong.
fn declared() -> EnvFile {
    let text = STACK
        .get_file(SETTINGS)
        .and_then(include_dir::File::contents_utf8)
        .unwrap_or_default();
    let settings = EnvFile::parse(text);
    assert!(
        !settings.keys().is_empty(),
        "the embedded stack declares no settings in {SETTINGS}, which means this is reading \
         the wrong thing"
    );
    settings
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
fn displayed_in_full(settings: &EnvFile) -> Vec<&str> {
    let supplied = a_value();
    settings
        .keys()
        .into_iter()
        .filter(|key| showing(key, &supplied).value == supplied)
        .collect()
}

/// Nothing is displayed with its value without a reason written down.
///
/// The default is the safe one and the exception is the thing that costs somebody
/// a sentence, which is the only arrangement that survives a setting being added
/// by whoever is in a hurry. A new setting holding a credential does not need to
/// be noticed in review: it is red until it is either named so the withholding
/// catches it, or argued for.
#[test]
fn no_setting_the_stack_declares_is_displayed_without_a_declared_reason() {
    let listed: BTreeSet<&str> = SHOWN.iter().map(|(key, _)| *key).collect();
    let settings = declared();
    let undecided: Vec<&str> = displayed_in_full(&settings)
        .into_iter()
        .filter(|key| !listed.contains(key))
        .collect();
    assert!(
        undecided.is_empty(),
        "these are served with their values to anything that reads the configuration, a \
         browser included, and nothing says why — give each a name the withholding \
         recognises, or add it to SHOWN with what makes displaying it safe: {undecided:?}"
    );
}

/// Nothing stays on the list after the stack stops displaying it.
///
/// The other direction of the same rule, and it is not the same test: the one
/// above starts from what the stack declares, so an entry for a setting that has
/// been dropped or has since been withheld would pass it in silence. An exception
/// that no longer excepts anything reads as a decision somebody made about the
/// stack as it is, and it is a decision about a stack that is gone.
#[test]
fn nothing_stays_on_the_shown_list_after_the_stack_stops_displaying_it() {
    let settings = declared();
    let displayed: BTreeSet<&str> = displayed_in_full(&settings).into_iter().collect();
    let stale: Vec<&str> = SHOWN
        .iter()
        .map(|(key, _)| *key)
        .filter(|key| !displayed.contains(key))
        .collect();
    assert!(
        stale.is_empty(),
        "these are written down as deliberately displayed and the stack displays neither \
         them nor their values — a setting it dropped, or one now withheld: {stale:?}"
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
        .map(|(key, _)| *key)
        .collect();
    assert!(
        silent.is_empty(),
        "these are displayed with their values and the list does not say what makes that \
         safe: {silent:?}"
    );
}

/// The module trees that draw a full screen, which is where a setting could reach
/// a terminal without passing the display path.
const DRAWING: [&str; 4] = ["acting", "dashboard", "pane.rs", "terminal.rs"];

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
    for (path, text) in source_tree::sources() {
        let where_it_lives = path.to_string_lossy().replace('\\', "/");
        let drawn = DRAWING
            .iter()
            .any(|tree| where_it_lives.contains(&format!("/src/{tree}")));
        if !drawn {
            continue;
        }
        for (number, line) in source_tree::production(&text).lines().enumerate() {
            if READING.iter().any(|how| line.contains(how)) {
                reaching.push(format!("{where_it_lives}:{}", number + 1));
            }
        }
    }

    assert!(
        reaching.is_empty(),
        "a screen reads settings by asking the core for them, never by reading them: \
         {reaching:?}"
    );
}
