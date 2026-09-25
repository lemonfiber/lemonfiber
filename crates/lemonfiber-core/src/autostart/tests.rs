use super::{Held, Returning, Wanted, DECLINE_CONSEQUENCE};

#[test]
fn declining_is_stated_as_what_it_costs_rather_than_what_it_is() {
    // "The stack will not start on boot" is a restatement of the question. An
    // operator weighing the answer is weighing four concrete things: when it
    // happens, what stops, that nothing tells them, and what it takes to undo.
    let said = DECLINE_CONSEQUENCE.to_lowercase();
    assert!(said.contains("restart"), "when it happens: {said}");
    assert!(said.contains("nothing comes back"), "what stops: {said}");
    assert!(
        said.contains("no notification"),
        "and nobody is told: {said}"
    );
    assert!(said.contains("lemonfiber up"), "the way back: {said}");
    // And it never leads with the property itself, which is the sentence that
    // reads as an explanation and explains nothing.
    assert!(!said.contains("autostart"), "a property: {said}");
    assert!(!said.contains("on boot"), "a property: {said}");
}

#[test]
fn the_answer_is_what_comes_back_out_of_the_file_it_is_kept_in() {
    // The whole of what this record is for. Gathering the answer and then
    // losing it is worse than never asking, because the operator believes they
    // have chosen something — which is the belief the feature exists to stop.
    for asked in [true, false] {
        let kept = serde_json::to_string(&Wanted::answered(asked)).unwrap_or_default();
        assert_eq!(
            serde_json::from_str::<Wanted>(&kept).ok(),
            Some(Wanted::answered(asked)),
            "{kept}"
        );
        assert_eq!(Wanted::answered(asked).on_boot(), asked);
    }
}

#[test]
fn what_comes_back_is_the_last_form_run_unless_one_is_pinned() {
    // The whole of the rule: an operator who ran `up films` last night gets
    // films back, and one who pinned `tv` gets tv whatever they ran by hand.
    let mut returning = Returning::default().answering(true);
    returning.started(&["films".to_owned()]);
    assert_eq!(
        returning.at_boot().ok(),
        Some(["films".to_owned()].as_slice())
    );

    returning.pin(&["tv".to_owned()]);
    assert_eq!(
        returning.at_boot().ok(),
        Some(["tv".to_owned()].as_slice()),
        "a pin stands ahead of whatever was run by hand"
    );

    returning.pin(&[]);
    assert_eq!(
        returning.at_boot().ok(),
        Some(["films".to_owned()].as_slice()),
        "and unpinning gives the last-run form back"
    );
}

#[test]
fn a_machine_that_has_run_nothing_yet_asks_for_every_form() {
    // Naming no form means the whole stack everywhere else in this product, and
    // it has to mean the same here: an operator who answered yes during setup and
    // then rebooted before ever running `up` is asking for their stack, not for
    // nothing at all.
    let returning = Returning::default().answering(true);
    assert_eq!(returning.at_boot().ok(), Some([].as_slice()));
}

#[test]
fn a_stack_stopped_on_purpose_is_not_resurrected() {
    // The edge case the requirement is written about. An operator who stopped the
    // stack on Friday and rebooted on Monday did not ask for it back.
    let mut returning = Returning::default().answering(true);
    returning.started(&["tv".to_owned()]);
    returning.stopped();

    assert_eq!(returning.at_boot(), Err(Held::StoppedOnPurpose));
    assert!(returning.halted());
    assert!(
        Held::StoppedOnPurpose.said().contains("on purpose"),
        "and the reason says which of the two it is"
    );
}

#[test]
fn starting_it_again_answers_the_stop() {
    // A deliberate stop is a statement about the stack as it was then. Starting it
    // is the operator saying otherwise, and a record that held the stop for ever
    // would leave a machine that never came back after any reboot.
    let mut returning = Returning::default().answering(true);
    returning.stopped();
    returning.started(&["tv".to_owned()]);

    assert!(!returning.halted());
    assert_eq!(returning.at_boot().ok(), Some(["tv".to_owned()].as_slice()));
}

#[test]
fn a_machine_that_was_never_asked_starts_nothing_however_it_was_left() {
    // And the two refusals are told apart, because putting them right takes
    // opposite things: answering the question, or starting the stack.
    let mut returning = Returning::default();
    returning.started(&["tv".to_owned()]);
    assert_eq!(returning.at_boot(), Err(Held::NotAsked));
    assert!(!Held::NotAsked.said().is_empty());
}

#[test]
fn the_answer_setup_wrote_before_any_of_this_existed_still_reads() {
    // The file format is the one already on operators' machines. A record holding
    // only the answer has to keep meaning what it meant, or an upgrade would
    // quietly forget every answer given so far.
    let read: Option<Returning> = serde_json::from_str(r#"{"on_boot":true}"#).ok();
    assert_eq!(
        read.as_ref().map(Returning::wanted),
        Some(Wanted::answered(true))
    );
    assert_eq!(
        read.and_then(|read| read.at_boot().ok().map(<[String]>::len)),
        Some(0)
    );
}

#[test]
fn the_answer_survives_setup_being_run_a_second_time() {
    // Setup writes the answer and nothing else, so a second run over a machine
    // that has been used must not take the pin and the stop with it.
    let mut returning = Returning::default().answering(true);
    returning.pin(&["tv".to_owned()]);
    returning.stopped();

    let again = returning.clone().answering(false);
    assert_eq!(again.pinned(), ["tv".to_owned()]);
    assert!(again.halted());
    assert_eq!(
        again.at_boot(),
        Err(Held::NotAsked),
        "and the new answer wins"
    );
}

#[test]
fn a_record_that_cannot_be_read_is_a_machine_that_was_never_asked() {
    // The safe direction, the same one a missing answer falls in: a truncated
    // file must not be the reason a metered line starts saturating at boot.
    let nowhere = lemonfiber_fixtures::scratch::Scratch::named("returning-absent.json");
    let _ = std::fs::remove_file(&nowhere);
    assert_eq!(Returning::at(&nowhere), Returning::default());

    assert!(std::fs::write(&nowhere, "not json at all").is_ok());
    assert_eq!(Returning::at(&nowhere).at_boot(), Err(Held::NotAsked));
    let _ = std::fs::remove_file(&nowhere);
}

#[test]
fn a_record_that_is_missing_the_answer_reads_as_not_asked_for() {
    // A record written before this field existed, and a record a later build
    // grew a second field onto, both arrive here. Neither may read as "they
    // asked for it" — starting a media stack nobody asked to start is the one
    // direction this must not fall in.
    assert_eq!(
        serde_json::from_str::<Wanted>("{}").ok(),
        Some(Wanted::default())
    );
    assert!(!Wanted::default().on_boot());
}
