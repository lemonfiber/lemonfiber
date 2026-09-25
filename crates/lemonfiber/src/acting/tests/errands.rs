//! Errands: a reset, a capture and a restore, asked and carried out.

use super::*;

/// What a reset would revert, or did.
fn a_reset(confirmed: bool) -> ResetReport {
    ResetReport {
        reverted: vec![StackEdit {
            path: "compose.yaml".to_owned(),
            diff: "-yours\n+ours".to_owned(),
        }],
        reverted_connections: vec!["sonarr → sabnzbd".to_owned()],
        confirmed,
    }
}

/// What a capture produced.
fn a_capture() -> backup::Report {
    backup::Report {
        path: PathBuf::from("/data/lemonfiber/backups/lemonfiber-full-1.tar.gz"),
        scope: Scope::WholeStack,
        sensitive: false,
        pruned: Vec::new(),
        pace: lemonfiber_core::backup::Pace::of(1_024),
        rehearsed: false,
    }
}

/// The other half of what this screen exists to prove: an errand takes the key
/// that opens the rest of them, a choice off that list, and an explicit yes
/// before anything reaches a command — and the command is one of the core's own.
#[test]
fn an_errand_takes_a_key_a_choice_and_an_answer_before_it_reaches_a_command() {
    let (mut acting, opened) = sending("seed");
    assert_eq!(opened, Wanted::Nothing);

    let said = showing(&acting);
    assert!(said.contains("Wire the services to each other?"), "{said}");

    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Seed)
    );
}

/// The claim the destructive errands are built around: what would be lost is on
/// the screen before the question is put, not after the answer is given.
#[test]
fn a_reset_says_what_it_would_throw_away_before_it_asks() {
    let (mut acting, weighing) = sending("reset");
    assert_eq!(weighing, Wanted::Carry(Command::Reset { confirm: false }));

    acting.came_to(Ok(Outcome::Reset(a_reset(false))));

    let said = showing(&acting);
    let before = said
        .split("Throw away every edit above?")
        .next()
        .unwrap_or_default();
    assert!(before.contains("sonarr → sabnzbd"), "{said}");
    assert!(before.contains("compose.yaml"), "{said}");
    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Reset { confirm: true })
    );
}

/// The same reading for a restore, which overwrites a configuration rather than
/// discarding edits: the archive is named, what it holds is read, and only then
/// is the overwrite agreed to.
#[test]
fn a_restore_is_named_read_and_only_then_agreed_to() {
    let (mut acting, opened) = sending("restore");
    assert_eq!(opened, Wanted::Nothing);
    assert!(showing(&acting).contains("Which backup"));

    for character in "lemonfiber-full-1.tar.gz".chars() {
        acting.pressed(&Press::Typed(character));
    }
    let listing = acting.pressed(&Press::Accept);

    assert_eq!(
        listing,
        Wanted::Carry(Command::Restore {
            archive: Kept::Named("lemonfiber-full-1.tar.gz".to_owned()),
            repoint: false,
            consent: restore::Consent::List,
        })
    );
}

/// A restore asked for with nothing typed is refused in the words the web
/// surface gives for the same request, rather than in a sentence written here.
#[test]
fn a_restore_with_no_name_says_what_is_missing() {
    let (mut acting, _) = sending("restore");

    assert_eq!(acting.pressed(&Press::Accept), Wanted::Nothing);

    let said = showing(&acting);
    assert!(said.contains("restore"), "{said}");
    assert!(said.contains("archive"), "{said}");
}

/// What an errand came to is shown in the words the command line gives for the
/// same run, and the screen behind it goes on being the report while it runs.
#[test]
fn an_errand_under_way_covers_nothing_and_says_what_it_came_to() {
    let (mut acting, _) = sending("backup");
    acting.pressed(&Press::Typed('y'));

    assert!(acting.pane(20, 100).is_none());
    let footing = footing(&acting);
    assert!(footing.contains("a backup"), "{footing}");
    assert!(
        acting
            .staying_for()
            .is_some_and(|said| said.contains("a backup")),
        "an errand with the core is waited for"
    );

    acting.came_to(Ok(Outcome::Backup(a_capture())));

    let said = showing(&acting);
    assert!(said.contains("Backed up"), "{said}");
}

/// An errand that failed says so in the command line's own words, the way an
/// action that failed does — a screen that went quiet would read as an errand
/// that never ran.
#[test]
fn an_errand_that_failed_says_why_in_the_words_the_command_line_gives() {
    let (mut acting, _) = sending("seed");
    acting.pressed(&Press::Typed('y'));

    acting.came_to(Err(Box::new(a_failure())));

    let said = showing(&acting);
    assert!(said.contains("could not be reached"), "{said}");
}

/// An errand with the core can be left, and leaving does not stop it — the same
/// reading a running action gets, because the same process holds the claim.
#[test]
fn leaving_while_an_errand_runs_leaves_the_screen_and_not_the_run() {
    let (mut acting, _) = sending("seed");
    acting.pressed(&Press::Typed('y'));

    assert_eq!(acting.pressed(&Press::Forward), Wanted::Nothing);
    assert_eq!(acting.pressed(&Press::Typed('q')), Wanted::Leave);
}

/// Anything that is not an explicit yes leaves the stack as it is, and the box
/// closes rather than staying open over a decision already taken.
#[test]
fn an_errand_answered_with_anything_else_changes_nothing() {
    let (mut acting, _) = sending("seed");

    assert_eq!(acting.pressed(&Press::Typed('n')), Wanted::Nothing);

    assert!(showing(&acting).is_empty());
}

/// What a reset would do moves under the arrows, and moving is not agreeing:
/// the question is still there afterwards and nothing has been sent.
#[test]
fn what_an_errand_would_do_moves_without_agreeing_to_it() {
    let (mut acting, _) = sending("reset");
    acting.came_to(Ok(Outcome::Reset(a_reset(false))));
    let opened = showing(&acting);

    assert_eq!(acting.pressed(&Press::Forward), Wanted::Nothing);

    let moved = showing(&acting);
    assert_ne!(moved, opened);
    assert!(moved.contains("Throw away every edit above?"), "{moved}");
    assert!(moved.contains("1 more line above"), "{moved}");
}

/// Backing out of the list, the line and the wait each leave the screen clear,
/// so no half-answered errand is left open behind whatever came next.
#[test]
fn backing_out_of_an_errand_leaves_the_screen_clear() {
    let mut acting = Acting::opened();
    acting.pressed(&Press::Typed(errand::KEY));
    assert_eq!(acting.pressed(&Press::Abandon), Wanted::Nothing);
    assert!(showing(&acting).is_empty());

    let (mut acting, _) = sending("restore");
    assert_eq!(acting.pressed(&Press::Abandon), Wanted::Nothing);
    assert!(showing(&acting).is_empty());

    let (mut acting, _) = sending("reset");
    assert_eq!(acting.pressed(&Press::Forward), Wanted::Nothing);
    assert!(showing(&acting).contains("working out what this would do"));
    assert_eq!(acting.pressed(&Press::Abandon), Wanted::Nothing);
    assert!(showing(&acting).is_empty());
}

/// Moving over the errands and typing at them take nothing, the way moving over
/// an action's subjects does: a stray key over a list is not an answer to it.
#[test]
fn moving_over_the_errands_and_typing_at_them_take_nothing() {
    let mut acting = Acting::opened();
    acting.pressed(&Press::Typed(errand::KEY));

    acting.pressed(&Press::Forward);
    acting.pressed(&Press::Back);
    assert_eq!(acting.pressed(&Press::Typed('y')), Wanted::Nothing);

    let said = showing(&acting);
    assert!(said.contains("> wiring"), "{said}");
}

/// The line an errand is named on ignores a move and takes back what was typed,
/// the way the line a question is typed on does.
#[test]
fn the_line_an_errand_is_named_on_takes_back_what_was_typed() {
    let (mut acting, _) = sending("restore");

    acting.pressed(&Press::Typed('a'));
    acting.pressed(&Press::Typed('b'));
    acting.pressed(&Press::Rubout);
    acting.pressed(&Press::Forward);

    let said = showing(&acting);
    assert!(said.contains("> a"), "{said}");
    assert!(!said.contains("> ab"), "{said}");
}

/// A stack that will not say what an errand would do ends the errand there:
/// there is nothing to agree to, and the failure is said in the command line's
/// own words.
#[test]
fn an_errand_the_stack_will_not_weigh_says_why_and_asks_nothing() {
    let (mut acting, _) = sending("reset");

    acting.came_to(Err(Box::new(a_failure())));

    let said = showing(&acting);
    assert!(said.contains("could not be reached"), "{said}");
    assert!(!said.contains("Throw away"), "{said}");
}

/// An errand naming an action no surface offers reaches no command and says so.
/// Nothing on the list is one, and that is what the guard beside the list holds;
/// this is the arm that would carry a name that stopped being offered.
#[test]
fn an_errand_naming_an_action_nothing_offers_says_so() {
    let mut acting = Acting::opened();

    let wanted = errand::agreeing(
        &mut acting.stage,
        &errand::tests::UNTRANSLATABLE,
        errand::Given::nothing(),
        None,
        &Press::Typed('y'),
    );

    assert_eq!(wanted, Wanted::Nothing);
    let said = showing(&acting);
    assert!(said.contains("There is no action named"), "{said}");
}

/// A restore onto a different data root is accepted at this screen now, and the
/// question that accepts it is the one under the listing that reported it.
///
/// The listing decides the question rather than the operator being asked in
/// advance: whether there is anything to accept is the archive's to say.
#[test]
fn a_restore_onto_a_different_data_root_is_accepted_under_the_listing() {
    let mut acting = naming_an_archive();
    acting.came_to(Ok(Outcome::Restore(a_restoration(Some(Relocation {
        was: "/srv/media".to_owned(),
        now: "/mnt/media".to_owned(),
    })))));

    let asked = showing(&acting);
    assert!(asked.contains("a different data root"), "{asked}");
    assert!(
        asked.contains("Restore from kept.tar.gz, re-pointing the data root"),
        "{asked}"
    );
    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Restore {
            archive: Kept::Named("kept.tar.gz".to_owned()),
            repoint: true,
            consent: restore::Consent::Standing,
        })
    );
}

/// An archive taken against this machine's own data root asks for no re-point and
/// sends none, which is what that field means on every surface.
#[test]
fn a_restore_that_moves_nothing_asks_for_no_re_point() {
    let mut acting = naming_an_archive();
    acting.came_to(Ok(Outcome::Restore(a_restoration(None))));

    let asked = showing(&acting);
    assert!(!asked.contains("re-pointing"), "{asked}");
    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Restore {
            archive: Kept::Named("kept.tar.gz".to_owned()),
            repoint: false,
            consent: restore::Consent::Standing,
        })
    );
}

/// The screen, having named an archive and sent the run that lists what it holds.
fn naming_an_archive() -> Acting {
    let (mut acting, _) = sending("restore");
    for character in "kept.tar.gz".chars() {
        acting.pressed(&Press::Typed(character));
    }
    acting.pressed(&Press::Accept);
    acting
}

/// What an archive says about itself before anything is overwritten, moved or not.
fn a_restoration(relocation: Option<Relocation>) -> Restoration {
    Restoration {
        would: Preview {
            manifest: Manifest {
                schema: SCHEMA,
                product_version: "0.9.0".to_owned(),
                created_at: "2026-08-26".to_owned(),
                data_root: "/srv/media".to_owned(),
                scope: Scope::WholeStack,
                sensitive: true,
                members: Vec::new(),
            },
            downgrade: false,
            relocation,
            agreement: "what this listing named itself".to_owned(),
        },
        done: None,
    }
}
