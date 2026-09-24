use super::{Cli, Mending, RawUnrated, Request};
use clap::{CommandFactory, Parser};

/// What `doctor` was asked to do about what it finds, for one command line.
///
/// Answered through the parser rather than by building the flags by hand, because the
/// question being asked is what a person typing this actually gets — including the
/// combinations the parser is meant to refuse. Nothing for a line that is not a
/// `doctor` run, or that the parser turns away.
fn doctoring(args: &[&str]) -> Option<(bool, Mending)> {
    match Cli::try_parse_from(args).ok()?.command? {
        Request::Doctor(asked) => Some((asked.disruptive, asked.mending)),
        _ => None,
    }
}

/// The forms a command line names, or nothing for a line that names none — a line
/// the parser turns away included, since a refused line named nothing either.
fn named_forms(args: &[&str]) -> Option<Vec<String>> {
    match Cli::try_parse_from(args).ok()?.command? {
        Request::Forms { forms } => Some(forms),
        _ => None,
    }
}

/// Asking what forms there are and asking what one of them would do are the same
/// word, told apart by what follows it — so the parser has to keep both open.
#[test]
fn asking_for_the_forms_and_asking_about_one_are_the_same_word() {
    assert_eq!(named_forms(&["lemonfiber", "forms"]), Some(Vec::new()));
    assert_eq!(
        named_forms(&["lemonfiber", "forms", "tv"]),
        Some(vec!["tv".to_owned()])
    );
    // Composition is asked about exactly as it is started.
    assert_eq!(
        named_forms(&["lemonfiber", "forms", "full", "proxy"]),
        Some(vec!["full".to_owned(), "proxy".to_owned()])
    );
    // A profile is an implementation detail, and no surface takes one.
    assert_eq!(
        named_forms(&["lemonfiber", "forms", "--profile", "media"]),
        None
    );
    assert_eq!(named_forms(&["lemonfiber", "version"]), None);
}

/// Looking and acting are told apart by what was asked for, not by which flag carries
/// it: a run that reverses a repair changes as much as one that makes it.
#[test]
fn a_run_that_changes_something_is_told_from_one_that_only_looks() {
    let acts = |args: &[&str]| doctoring(args).map(|(_, mending)| mending.acts());

    assert_eq!(acts(&["lemonfiber", "doctor"]), Some(false));
    assert_eq!(acts(&["lemonfiber", "doctor", "--fix"]), Some(true));
    assert_eq!(acts(&["lemonfiber", "doctor", "--undo"]), Some(true));
    // The question is doctor's alone — every other command already says what it does.
    assert_eq!(acts(&["lemonfiber", "seed"]), None);
}

/// Repairing and reversing a repair in one run is not a thing to guess the order of,
/// so it is refused at the parser rather than resolved somewhere further in.
#[test]
fn repairing_and_reversing_at_once_is_refused() {
    assert!(doctoring(&["lemonfiber", "doctor", "--fix", "--undo"]).is_none());
}

/// What an invitation was told somebody may watch, for one command line.
///
/// Answered through the parser for the reason a `doctor` run is: what is under test
/// is what a person typing this gets, including the lines the parser turns away.
fn inviting(args: &[&str]) -> Option<(String, Vec<String>, Option<u32>)> {
    match Cli::try_parse_from(args).ok()?.command? {
        Request::Invite { name, allowance } => {
            Some((name, allowance.libraries, allowance.age_limit))
        }
        _ => None,
    }
}

/// An invitation that names neither chooses neither, which is the ordinary case:
/// every library, and no age limit.
#[test]
fn an_invitation_that_chooses_nothing_carries_nothing() {
    assert_eq!(
        inviting(&["lemonfiber", "invite", "ana"]),
        Some(("ana".to_owned(), Vec::new(), None))
    );
}

/// Several libraries are named one flag at a time, the way named services are, so
/// the list cannot run on into the name the invitation is for.
#[test]
fn libraries_are_named_one_at_a_time_and_do_not_swallow_the_name() {
    assert_eq!(
        inviting(&[
            "lemonfiber",
            "invite",
            "--library",
            "Films",
            "--library",
            "Shows",
            "ana",
        ]),
        Some((
            "ana".to_owned(),
            vec!["Films".to_owned(), "Shows".to_owned()],
            None
        ))
    );
}

/// The age limit is carried as the age it was typed as, because the media server
/// keeps an age and there is nothing to translate.
#[test]
fn the_age_limit_is_carried_as_the_age_it_was_typed_as() {
    assert_eq!(
        inviting(&["lemonfiber", "invite", "ana", "--age-limit", "12"]),
        Some(("ana".to_owned(), Vec::new(), Some(12)))
    );
}

/// A limit that is not an age at all is refused at the parser rather than sent to
/// the media server to be refused as something else.
#[test]
fn a_limit_that_is_not_an_age_is_refused() {
    assert_eq!(
        inviting(&["lemonfiber", "invite", "ana", "--age-limit", "PG"]),
        None
    );
}

/// The reader answers for an invitation and for nothing else.
///
/// Every other command parses perfectly well and carries no allowance, so the
/// helper above has a case for them — and a case nothing reaches is a case that
/// could say anything.
#[test]
fn a_command_that_is_not_an_invitation_carries_no_allowance() {
    assert_eq!(inviting(&["lemonfiber", "household"]), None);
}

/// What to do about unrated content is one of two words, and it reaches the
/// request as the one that was typed.
#[test]
fn what_to_do_about_unrated_content_is_carried_as_the_word_it_was_typed_as() {
    for (typed, chosen) in [("block", RawUnrated::Block), ("allow", RawUnrated::Allow)] {
        assert_eq!(
            unrating(&["lemonfiber", "invite", "ana", "--unrated", typed]),
            Some(("ana".to_owned(), Some(chosen))),
            "{typed}"
        );
    }
}

/// A third word is refused at the parser rather than sent on to mean whichever
/// answer the far side happened to default to.
#[test]
fn a_third_word_about_unrated_content_is_refused() {
    assert_eq!(
        unrating(&["lemonfiber", "invite", "ana", "--unrated", "hide"]),
        None
    );
}

/// Saying nothing about it says nothing, which leaves the answer to the core.
///
/// And the reader answers for an invitation and for nothing else: every other
/// command parses perfectly well and carries no word, so the case for them is here
/// rather than left as a case nothing reaches.
#[test]
fn saying_nothing_about_unrated_content_carries_nothing() {
    assert_eq!(
        unrating(&["lemonfiber", "invite", "ana"]),
        Some(("ana".to_owned(), None)),
        "a word nobody typed was carried anyway"
    );
    assert_eq!(unrating(&["lemonfiber", "household"]), None);
}

/// Who an invitation is for and what it was told to do about unrated content, for
/// one command line.
///
/// The name comes back beside the word so that nothing is the *absence* of a
/// parse: a line the parser turned away answers with nothing at all, and one that
/// named no word answers with the name and no word.
fn unrating(args: &[&str]) -> Option<(String, Option<RawUnrated>)> {
    match Cli::try_parse_from(args).ok()?.command? {
        Request::Invite { name, allowance } => Some((name, allowance.unrated)),
        _ => None,
    }
}

/// The steps the flag's own help names are the steps the core offers.
///
/// Held rather than trusted, because the help is a sentence in a doc comment and
/// the steps are a table somewhere else: a step added to one and not the other is
/// a number an operator is either never told about or told about wrongly.
#[test]
fn the_help_names_every_step_the_core_offers() {
    let help = Cli::command()
        .find_subcommand_mut("invite")
        .map(|invite| invite.render_long_help().to_string())
        .unwrap_or_default();

    assert!(!help.is_empty(), "the invite command has no help to read");
    for step in lemonfiber_core::age_limit::steps() {
        assert!(
            help.contains(&step.age.to_string()),
            "{} is a step the core offers and the help does not name",
            step.age
        );
    }
}

/// Two flags that read differently on the command line must be two arguments
/// underneath. `doctor` has a `--disruptive` of its own, and a repairing run has
/// `--fix-disruptive`; keyed by field name they would collide, and the one that lost
/// would silently do nothing.
#[test]
fn disturbing_the_stack_while_repairing_is_its_own_flag() {
    let disturbs =
        |args: &[&str]| doctoring(args).map(|(all, mending)| (all, mending.fixing.disruptive));

    assert_eq!(
        disturbs(&["lemonfiber", "doctor", "--disruptive"]),
        Some((true, false))
    );
    assert_eq!(
        disturbs(&["lemonfiber", "doctor", "--fix", "--fix-disruptive"]),
        Some((false, true))
    );
}
