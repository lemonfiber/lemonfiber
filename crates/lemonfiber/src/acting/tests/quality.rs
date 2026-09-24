//! Changing the quality choice, and what it costs first.

use super::*;

/// The screen, having taken the change that records a choice and then the media
/// that choice is about.
///
/// Two steps rather than one, because what a bar even is depends on the first:
/// music has no resolution, so its list is three audio formats where the rest are
/// four presets.
fn aiming(about: &str) -> Acting {
    let (mut acting, _) = changing("quality-set");
    onto(&mut acting, about);
    acting.pressed(&Press::Accept);
    acting
}

/// The quality in force as the read reports it, and what became of a choice.
///
/// The preset's own words rather than a paraphrase, so what is asserted about
/// the screen is what the core would really have put on it.
fn a_quality(disposition: Disposition) -> QualityReport {
    let cost = Preset::Maximum.consequence();
    QualityReport {
        choices: vec![PresetChoice {
            scope: "everything".to_owned(),
            preset: Preset::Maximum.label().to_owned(),
            means: Preset::Maximum.means().to_owned(),
            resolution: cost.resolution.to_owned(),
            size_per_hour: cost.size_per_hour.to_owned(),
            transcoding: cost.transcoding.to_owned(),
            needs_transcoding_here: true,
        }],
        music: None,
        customised: false,
        overwritten: None,
        disposition,
    }
}

/// What an upgrade would cost, having triggered nothing.
fn an_upgrade() -> UpgradeReport {
    UpgradeReport {
        confirmed: false,
        media: vec![UpgradeMedia {
            media_type: "television".to_owned(),
            preset: Preset::Balanced.label().to_owned(),
            size_per_hour: Preset::Balanced.consequence().size_per_hour.to_owned(),
            outcome: None,
        }],
    }
}

/// The same claim as the actions and the errands, on the third list: a key, a
/// choice, a question and an explicit yes before anything reaches a command —
/// and the command is the one every other surface produces for that request.
#[test]
fn a_quality_change_takes_a_key_a_choice_and_an_answer_before_it_reaches_a_command() {
    let (mut acting, opened) = changing("quality-set");
    assert_eq!(opened, Wanted::Nothing);

    // The media a choice is about comes first, and the whole library is the row
    // the list opens on — so an operator who presses enter twice makes the choice
    // this screen has always made.
    let media = showing(&acting);
    assert!(media.contains("> everything"), "{media}");
    assert!(media.contains("music"), "{media}");

    acting.pressed(&Press::Accept);
    let listed = showing(&acting);
    assert!(listed.contains("> space-saving"), "{listed}");
    assert!(listed.contains("per hour"), "{listed}");

    acting.pressed(&Press::Forward);
    assert_eq!(acting.pressed(&Press::Accept), Wanted::Nothing);
    let asked = showing(&acting);
    assert!(asked.contains("Aim for balanced, everywhere?"), "{asked}");

    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Quality(QualityAction::Set {
            preset: Preset::Balanced,
            media_type: None,
            confirm: false,
        }))
    );
}

/// What an upgrade would cost is on the screen before the question is put, not
/// after the answer is given — the reading a reset is already offered under,
/// arriving on the one of these three whose unconfirmed run really does report
/// and change nothing.
#[test]
fn what_an_upgrade_would_cost_is_read_before_it_is_agreed_to() {
    let (mut acting, costing) = changing("quality-upgrade");
    assert_eq!(
        costing,
        Wanted::Carry(Command::QualityUpgrade { confirm: false })
    );

    acting.came_to(Ok(Outcome::Upgrade(an_upgrade())));

    let said = showing(&acting);
    let before = said
        .split("Fetch everything again at the quality chosen?")
        .next()
        .unwrap_or_default();
    assert!(before.contains("television"), "{said}");
    assert!(
        before.contains(Preset::Balanced.consequence().size_per_hour),
        "{said}"
    );
    assert!(before.contains("Nothing has been changed"), "{said}");
    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::QualityUpgrade { confirm: true })
    );

    acting.came_to(Ok(Outcome::Upgrade(UpgradeReport {
        confirmed: true,
        media: an_upgrade().media,
    })));

    let came = showing(&acting);
    assert!(came.contains("Upgrading existing content"), "{came}");
    assert!(!came.contains("Fetch everything again"), "{came}");
}

/// A choice this host could only transcode in software comes back held rather
/// than recorded, and the caution it comes back with is what the second question
/// sits under. The agreement goes on that second send and never on the first.
#[test]
fn a_choice_this_host_would_transcode_is_held_and_the_caution_is_read_first() {
    let mut acting = aiming("everything");
    onto(&mut acting, "maximum");
    acting.pressed(&Press::Accept);
    let first = acting.pressed(&Press::Typed('y'));
    assert_eq!(
        first,
        Wanted::Carry(Command::Quality(QualityAction::Set {
            preset: Preset::Maximum,
            media_type: None,
            confirm: false,
        }))
    );

    acting.came_to(Ok(Outcome::Quality(a_quality(Disposition::Held))));

    let said = showing(&acting);
    let before = said
        .split("Aim for maximum, everywhere?")
        .next()
        .unwrap_or_default();
    assert!(before.contains("transcode this in software"), "{said}");
    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Quality(QualityAction::Set {
            preset: Preset::Maximum,
            media_type: None,
            confirm: true,
        }))
    );
}

/// A choice that was recorded is read and put away, not asked about again — the
/// second question exists for the one cost the agreement is for and for nothing
/// else.
#[test]
fn a_choice_that_was_recorded_is_read_rather_than_asked_about_again() {
    let mut acting = aiming("everything");
    acting.pressed(&Press::Accept);
    acting.pressed(&Press::Typed('y'));

    acting.came_to(Ok(Outcome::Quality(a_quality(Disposition::Recorded))));

    let said = showing(&acting);
    assert!(said.contains("Saved."), "{said}");
    assert!(!said.contains("Aim for"), "{said}");
    assert_eq!(acting.pressed(&Press::Typed('y')), Wanted::Nothing);
}

/// Re-asserting carries no agreement and has no half that only reports, so the
/// question is put with nothing above it rather than with a preamble invented
/// for symmetry — and what it would overwrite is the line under the question.
#[test]
fn re_asserting_the_choice_is_asked_about_with_nothing_run_first() {
    let (mut acting, opened) = changing("quality-reapply");
    assert_eq!(opened, Wanted::Nothing);

    let said = showing(&acting);
    assert!(
        said.contains("Overwrite your own edits with the chosen preset?"),
        "{said}"
    );
    assert!(said.contains("edited by hand"), "{said}");
    assert!(!said.contains("more line"), "{said}");

    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Quality(QualityAction::Reapply))
    );
}

/// Moving over either quality list lands where it started, and a stray character
/// pressed over one is not an answer to it.
#[test]
fn moving_over_the_quality_lists_and_typing_at_them_take_nothing() {
    let mut acting = Acting::opened();
    acting.pressed(&Press::Typed(quality::KEY));

    acting.pressed(&Press::Forward);
    acting.pressed(&Press::Back);
    acting.pressed(&Press::Rubout);
    assert_eq!(acting.pressed(&Press::Typed('y')), Wanted::Nothing);
    assert!(showing(&acting).contains("> how good it should be"));

    acting.pressed(&Press::Accept);
    acting.pressed(&Press::Forward);
    acting.pressed(&Press::Back);
    acting.pressed(&Press::Rubout);
    acting.pressed(&Press::Typed('z'));
    assert!(showing(&acting).contains("> everything"));

    acting.pressed(&Press::Accept);
    acting.pressed(&Press::Forward);
    acting.pressed(&Press::Back);
    acting.pressed(&Press::Rubout);
    acting.pressed(&Press::Typed('z'));
    assert!(showing(&acting).contains("> space-saving"));

    acting.pressed(&Press::Abandon);
    assert_eq!(showing(&acting), "");
}

/// Backing out of the list of changes leaves nothing open, and a question left
/// alone leaves the quality as it was.
#[test]
fn backing_out_of_a_quality_change_leaves_it_as_it_was() {
    let mut acting = Acting::opened();
    acting.pressed(&Press::Typed(quality::KEY));
    acting.pressed(&Press::Abandon);
    assert_eq!(showing(&acting), "");

    let (mut acting, _) = changing("quality-reapply");
    assert_eq!(acting.pressed(&Press::Typed('n')), Wanted::Nothing);
    assert_eq!(showing(&acting), "");
}

/// The account in front of the question moves, because a cost stated per media
/// type is longer than the box over a dashboard and one whose end nobody can
/// reach is one nobody has read.
#[test]
fn the_account_in_front_of_the_question_moves() {
    let (mut acting, _) = changing("quality-upgrade");
    acting.came_to(Ok(Outcome::Upgrade(an_upgrade())));
    let opened = showing(&acting);

    acting.pressed(&Press::Forward);

    assert_ne!(showing(&acting), opened);
    assert!(showing(&acting).contains("1 more line above"));
    acting.pressed(&Press::Back);
    assert_eq!(showing(&acting), opened);
}

/// While a cost is with the core the screen says so, and backing out then leaves
/// nothing open — the answer that arrives afterwards has nobody waiting for it.
#[test]
fn a_cost_being_worked_out_is_said_and_can_be_backed_out_of() {
    let (mut acting, _) = changing("quality-upgrade");
    let said = showing(&acting);
    assert!(said.contains("working out what this would cost"), "{said}");

    assert_eq!(acting.pressed(&Press::Typed('y')), Wanted::Nothing);
    assert_eq!(showing(&acting), said);

    acting.pressed(&Press::Abandon);
    assert_eq!(showing(&acting), "");
    acting.came_to(Ok(Outcome::Upgrade(an_upgrade())));
    assert_eq!(showing(&acting), "");
}

/// A change that is running draws no box, says what it is on the footer, and
/// tells a run leaving now what it would be waiting for.
#[test]
fn a_running_quality_change_is_on_the_footer_and_leaving_says_what_it_waits_for() {
    let (mut acting, _) = changing("quality-reapply");
    acting.pressed(&Press::Typed('y'));

    assert_eq!(showing(&acting), "");
    let footing = footing(&acting);
    assert!(
        footing.contains("the choice put back over your edits"),
        "{footing}"
    );
    assert!(footing.contains("still running"), "{footing}");
    let leaving = acting.staying_for().unwrap_or_default();
    assert!(leaving.contains("leave the stack claimed"), "{leaving}");

    assert_eq!(acting.pressed(&Press::Typed('z')), Wanted::Nothing);
    assert_eq!(acting.pressed(&Press::Typed('q')), Wanted::Leave);
}

/// A failure on either half is reported in the words the command line gives for
/// the same one, and nothing is left to agree to.
#[test]
fn a_failure_while_a_quality_change_is_with_the_core_is_reported() {
    let (mut acting, _) = changing("quality-upgrade");
    acting.came_to(Err(Box::new(a_failure())));
    let costing = showing(&acting);
    assert!(costing.contains("could not be reached"), "{costing}");
    assert!(!costing.contains("Fetch everything again"), "{costing}");

    let (mut acting, _) = changing("quality-reapply");
    acting.pressed(&Press::Typed('y'));
    acting.came_to(Err(Box::new(a_failure())));
    let applying = showing(&acting);
    assert!(applying.contains("could not be reached"), "{applying}");
}

/// A change naming an action no surface offers reaches no command and says so,
/// on each of the three paths that ask the translation for one. Nothing on the
/// list is one, and the guard beside the list is what holds that; these are the
/// arms that would carry a name which stopped being offered.
#[test]
fn a_quality_change_naming_an_action_nothing_offers_says_so() {
    for change in [&quality::tests::UNCHOOSABLE, &quality::tests::UNCOSTABLE] {
        let mut acting = Acting::opened();

        let wanted = quality::deciding(
            &mut acting.stage,
            Chooser::over(change, Vec::new()),
            &Press::Accept,
        );

        assert_eq!(wanted, Wanted::Nothing);
        let said = showing(&acting);
        assert!(said.contains("There is no action named"), "{said}");
    }

    let mut acting = Acting::opened();
    let wanted = quality::settling(
        &mut acting.stage,
        &quality::tests::UNCHOOSABLE,
        quality::Chosen::nothing(),
        None,
        &Press::Typed('y'),
    );

    assert_eq!(wanted, Wanted::Nothing);
    assert!(showing(&acting).contains("There is no action named"));
}

/// A quality choice is narrowed to one kind of media, which is the step in front
/// of the bars rather than an argument nobody can name.
///
/// The media reaches the command as the word `--for` spells and the bar reaches it
/// as the preset, so what is asserted is the pair — which is what a screen that
/// carried only one of them would get wrong.
#[test]
fn a_quality_choice_is_narrowed_to_one_kind_of_media() {
    let mut acting = aiming("series");
    onto(&mut acting, "high-quality");
    acting.pressed(&Press::Accept);

    let asked = showing(&acting);
    assert!(
        asked.contains("Aim for high-quality, for series?"),
        "{asked}"
    );
    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Quality(QualityAction::Set {
            preset: Preset::HighQuality,
            media_type: Some("tv".to_owned()),
            confirm: false,
        }))
    );
}

/// Choosing for music offers audio formats rather than resolutions, and reaches
/// the command that fork ends in.
///
/// The bars are not filtered here and no list of them is written down twice: every
/// preset and every format is put to the translation for this media, and what
/// comes back as a command is what is shown. A screen keeping its own idea of
/// which is which would offer a resolution for music and fail here.
#[test]
fn choosing_for_music_offers_audio_formats_and_reaches_its_own_command() {
    let mut acting = aiming("music");

    let bars = showing(&acting);
    assert!(bars.contains("lossless"), "{bars}");
    assert!(!bars.contains("space-saving"), "{bars}");
    assert!(!bars.contains("maximum"), "{bars}");

    onto(&mut acting, "hi-res");
    acting.pressed(&Press::Accept);
    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::QualityMusic {
            format: Format::HiRes,
        })
    );
}

/// A trace is narrowed to one season, which is a second line under the first.
///
/// The title stays on the screen while the season is typed, because the second
/// line is about the first and a box that had taken it away would be asking
/// somebody to remember what they were narrowing.
#[test]
fn a_trace_is_narrowed_to_one_season_on_a_second_line() {
    let mut acting = asking("where one season of it is");
    for character in "The Expanse".chars() {
        acting.pressed(&Press::Typed(character));
    }
    assert_eq!(acting.pressed(&Press::Accept), Wanted::Nothing);

    let second = showing(&acting);
    assert!(second.contains("Which season, as a number"), "{second}");
    assert!(second.contains("> The Expanse"), "{second}");

    acting.pressed(&Press::Typed('2'));
    assert_eq!(
        acting.pressed(&Press::Accept),
        Wanted::Carry(Command::Trace {
            term: "The Expanse".to_owned(),
            season: Some(2),
            searching: false,
        })
    );
}
