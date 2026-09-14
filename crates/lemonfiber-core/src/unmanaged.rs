//! Areas the operator has told lemonfiber to leave alone.
//!
//! An experienced operator who wants to keep one part of the stack by hand should be
//! able to say so and be obeyed, rather than fight the same file back into shape after
//! every run. Three mechanisms already infer something like it — a value with no
//! baseline is observed rather than written over, a drifted value can be adopted so
//! lemonfiber stops pushing its own default, and a doctor check can be accepted so it
//! stops being reported — and none of them is this one. All three are per-field, and
//! all three are reachable only for values lemonfiber already wires. None can cover a
//! compose file, a directory of configuration, or a whole service.
//!
//! This is the declaration itself: a list of names, each with the reason somebody gave
//! for writing it down. It is kept in the settings file, beside the register of admin
//! services deliberately exposed, and for the same reasons — it is a decision, it
//! travels with the configuration it is about, a backup that takes one takes the other,
//! and its whole worth is that a person can read it back in a year.
//!
//! # What a name covers
//!
//! One rule, so there is one thing to learn: a name covers the thing it spells, and
//! anything beneath it separated by a slash. `sonarr` covers the service. `config` and
//! `config/recyclarr` both cover `config/recyclarr/recyclarr.yml`. `DATA_ROOT` covers
//! that setting. Nothing is inferred from the shape of a name — a name is not parsed to
//! decide whether it is a service or a path, because a fork may name a service
//! `config`, and a rule that guessed would be wrong in whichever direction nobody chose.
//!
//! # What it stops, and what it does not
//!
//! **This is the honest half, and it has to be read before anybody relies on it.** A
//! declaration stops lemonfiber writing on its own account:
//!
//! - a materialised stack file beneath a declared name is not written, on any run;
//! - a service with a declared name is not seeded, and is reported as observed rather
//!   than wired;
//! - a setting with a declared name is refused by `config set`, with the reason the
//!   operator gave.
//!
//! It does **not** stop a repair the operator confirms. A repair is consented to one at
//! a time, by name, after being shown what it would do, and it is already held back
//! from anything lemonfiber did not write and still owns — so what would be gained by
//! refusing it here is a second opinion about a decision somebody has just taken. That
//! is a deliberate limit rather than an oversight, and it is stated where the setting is
//! described so that nobody reads this as a promise it does not make.
//!
//! # What makes an entry a declaration, and why the bar is lower here
//!
//! A name and a reason, and the reason has only to be there. The register of
//! deliberately exposed services beside this asks for four words, and the asymmetry is
//! deliberate: that one *silences a warning*, so an entry too thin to be a decision
//! must not count, and the cost of dropping it is that the warning goes on being
//! reported. This one *stops lemonfiber writing*, so an entry that is dropped is an
//! operator who wrote something down, watched their file be rewritten anyway, and has
//! nothing anywhere telling them their sentence was too short. Being misled about what
//! is protected is the failure this mechanism must not have, and it is worth a weaker
//! record to avoid.
//!
//! An entry with no `=` at all declares nothing, and that is the one silence left. It
//! is a syntax error rather than a judgement about somebody's wording, and it reads as
//! one where the setting is shown back to them.

/// Every area declared unmanaged, as pairs of name and reason.
///
/// Parsed out of the settings file's one line, the way the exposed register is: pairs
/// of `name=reason`, comma-separated. Both halves have to be there and neither may be
/// blank; how good the reason is is not judged here, for the reason written at the top
/// of this module.
#[must_use]
pub fn parse(written: &str) -> Vec<(String, String)> {
    written
        .split(',')
        .filter_map(|entry| entry.split_once('='))
        .map(|(area, why)| (area.trim().to_owned(), why.trim().to_owned()))
        .filter(|(area, why)| !area.is_empty() && !why.is_empty())
        .collect()
}

/// The reason this thing is unmanaged, where it is — the declaration that covers it.
///
/// The reason rather than a yes-or-no, because every caller that has to stop also has
/// to say why it stopped, and "the operator said so" without their words is the half of
/// the answer nobody can act on.
///
/// The first declaration that covers it, where more than one does. Two entries covering
/// one thing is an operator writing the same decision twice rather than two decisions,
/// and picking the narrower would be inventing a precedence nobody asked for.
#[must_use]
pub fn covering<'a>(declared: &'a [(String, String)], what: &str) -> Option<&'a str> {
    declared
        .iter()
        .find(|(area, _)| beneath(area, what))
        .map(|(_, why)| why.as_str())
}

/// Whether anything declared covers this.
#[must_use]
pub fn covers(declared: &[(String, String)], what: &str) -> bool {
    covering(declared, what).is_some()
}

/// Whether `what` is the declared area, or sits beneath it.
///
/// Beneath means separated by a slash, which is what keeps `config` from covering
/// `configuration.yml`: a name that merely starts with another name is a different name,
/// and a declaration that swallowed its neighbours would stop lemonfiber maintaining
/// files nobody said anything about.
fn beneath(area: &str, what: &str) -> bool {
    what == area
        || what
            .strip_prefix(area)
            .is_some_and(|rest| rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::{covering, covers, parse};

    /// One declaration, with a reason long enough to be one.
    fn declared(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(area, why)| ((*area).to_owned(), (*why).to_owned()))
            .collect()
    }

    #[test]
    fn a_declaration_is_a_name_and_the_reason_somebody_gave() {
        let read = parse("sonarr=I tune this one by hand every season");
        assert_eq!(
            read,
            declared(&[("sonarr", "I tune this one by hand every season")])
        );
    }

    #[test]
    fn several_declarations_are_separated_by_commas_and_trimmed() {
        let read = parse(
            " sonarr = I tune this one by hand every season , \
             config/recyclarr = my own profiles live in here ",
        );
        assert_eq!(read.len(), 2, "{read:?}");
        assert!(read.iter().any(|(area, _)| area == "config/recyclarr"));
    }

    /// Both halves, and neither blank. How good the reason is is not judged: an entry
    /// dropped here is somebody who wrote something down, watched their file be
    /// rewritten anyway, and has nothing telling them their sentence was too short.
    #[test]
    fn an_entry_missing_either_half_declares_nothing() {
        assert!(parse("sonarr").is_empty(), "no reason at all");
        assert!(parse("sonarr=").is_empty(), "a blank reason");
        assert!(parse("=I have my reasons").is_empty(), "no name");
        assert!(parse("").is_empty(), "nothing written down");
    }

    /// And a thin reason still declares, which is the asymmetry with the register of
    /// deliberately exposed services: that one silences a warning and this one stops a
    /// write, so they fail in opposite directions on purpose.
    #[test]
    fn a_reason_nobody_would_praise_is_still_a_declaration() {
        assert_eq!(parse("sonarr=mine"), declared(&[("sonarr", "mine")]));
    }

    #[test]
    fn a_declared_name_covers_itself_and_says_why() {
        let theirs = declared(&[("sonarr", "I tune this one by hand every season")]);
        assert_eq!(
            covering(&theirs, "sonarr"),
            Some("I tune this one by hand every season")
        );
        assert!(covers(&theirs, "sonarr"));
    }

    #[test]
    fn a_declared_name_covers_what_sits_beneath_it() {
        let theirs = declared(&[("config/recyclarr", "my own profiles live in here")]);
        assert!(covers(&theirs, "config/recyclarr/recyclarr.yml"));
        assert!(covers(&theirs, "config/recyclarr/includes/mine.yml"));
    }

    /// A name that merely begins with another name is a different name.
    #[test]
    fn a_declared_name_does_not_cover_its_neighbours() {
        let theirs = declared(&[("config", "everything under here is mine to keep")]);
        assert!(!covers(&theirs, "configuration.yml"));
        assert!(!covers(&theirs, "config.yml"));
        assert!(covers(&theirs, "config/recyclarr/recyclarr.yml"));
    }

    #[test]
    fn nothing_declared_covers_nothing() {
        assert!(!covers(&[], "sonarr"));
        assert_eq!(covering(&[], "sonarr"), None);
    }

    /// Nothing is inferred from the shape of a name, so a service and a path are the
    /// same kind of thing to this — which is what lets a fork call a service `config`.
    #[test]
    fn a_name_is_a_name_whether_it_looks_like_a_path_or_a_service() {
        let theirs = declared(&[
            ("DATA_ROOT", "I move the library about by hand"),
            ("compose.yml", "I keep my own edits in this file"),
        ]);
        assert!(covers(&theirs, "DATA_ROOT"));
        assert!(covers(&theirs, "compose.yml"));
        assert!(!covers(&theirs, "DATA_ROOT_BACKUP"));
    }
}
