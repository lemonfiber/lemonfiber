//! Acting on some of a form's services rather than the whole form.

use super::*;

/// Some of a form's services are started rather than the whole of it, which is
/// the argument this screen had no way to name: the form is taken off the list it
/// always had, the services inside it are marked on the list beside that one, and
/// what goes ahead is the command the command line reaches for `up --service`.
///
/// Whether that is `Up` or `Start` is not this screen's to know. The names go to
/// the same translation a browser's do and whatever comes back is what is
/// carried, which is why the fork Compose spells two ways costs the screen no
/// second flow.
#[test]
fn some_of_a_forms_services_are_started_rather_than_the_whole_form() {
    let mut acting = holding('u');
    onto(&mut acting, "Full stack");
    acting.pressed(&Press::Accept);
    let inside = showing(&acting);
    onto(&mut acting, "Sonarr");
    acting.pressed(&Press::Typed(' '));
    acting.pressed(&Press::Accept);
    let question = showing(&acting);

    let wanted = acting.pressed(&Press::Typed('y'));

    assert!(
        inside.contains("Sonarr") && inside.contains("healthy"),
        "{inside}"
    );
    assert!(question.contains("Start Sonarr?"), "{question}");
    assert_eq!(
        wanted,
        Wanted::Carry(Command::Start {
            forms: vec!["full".to_owned()],
            services: vec!["sonarr".to_owned()],
        })
    );
}

/// Stopping some of them is the same three presses, and reaches the command the
/// command line reaches for `down --service` — which is a different command
/// again, and still nothing this screen chose between.
#[test]
fn some_of_a_forms_services_are_stopped_rather_than_the_whole_form() {
    let mut acting = holding('d');
    onto(&mut acting, "Full stack");
    acting.pressed(&Press::Accept);
    onto(&mut acting, "Radarr");
    acting.pressed(&Press::Accept);

    let question = showing(&acting);

    assert!(question.contains("Stop Radarr?"), "{question}");
    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Halt {
            forms: vec!["full".to_owned()],
            services: vec!["radarr".to_owned()],
        })
    );
}

/// A restart names the services `--service` restarts, and keeps the forms it was
/// already given — which is what that command insists on.
#[test]
fn a_restart_names_the_services_it_restarts_inside_the_form_it_was_given() {
    let mut acting = holding('t');
    onto(&mut acting, "Full stack");
    acting.pressed(&Press::Accept);
    onto(&mut acting, "Sonarr");
    acting.pressed(&Press::Accept);

    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Restart {
            forms: vec!["full".to_owned()],
            services: vec!["sonarr".to_owned()],
        })
    );
}

/// An operator who wants the whole form still gets the question they always got,
/// named by the form rather than by a row saying every service.
#[test]
fn naming_no_service_asks_about_the_form_it_always_asked_about() {
    let mut acting = holding('u');
    onto(&mut acting, "Full stack");
    acting.pressed(&Press::Accept);
    acting.pressed(&Press::Accept);

    let question = showing(&acting);

    assert!(question.contains("Start Full stack?"), "{question}");
    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Up {
            forms: vec!["full".to_owned()],
        })
    );
}

/// The two that carry no service are not offered a list of them, so their flow is
/// the one they had — which is what keeps the screen from asking for something no
/// other surface can ask for.
#[test]
fn an_action_that_carries_no_service_goes_straight_to_its_question() {
    let mut acting = holding('p');
    onto(&mut acting, "Full stack");
    acting.pressed(&Press::Accept);

    let question = showing(&acting);

    assert!(
        question.contains("Fetch newer images for Full stack?"),
        "{question}"
    );
}

/// The capture names one service by taking it off the list rather than by having
/// it typed. A typed one would be a name nothing checked before the capture ran.
#[test]
fn a_capture_names_one_service_by_taking_it_off_the_list() {
    let mut acting = Acting::opened();
    acting.gathered(&super::super::service::tests::two_services());
    acting.pressed(&Press::Typed(errand::KEY));
    onto(&mut acting, &errand::tests::listed("backup"));
    acting.pressed(&Press::Accept);
    let inside = showing(&acting);
    onto(&mut acting, "Sonarr");
    acting.pressed(&Press::Accept);
    let question = showing(&acting);

    let wanted = acting.pressed(&Press::Typed('y'));

    assert!(inside.contains("the whole stack"), "{inside}");
    assert!(
        question.contains("Capture the configuration of Sonarr?"),
        "{question}"
    );
    assert_eq!(
        wanted,
        Wanted::Carry(Command::Backup {
            service: Some("sonarr".to_owned()),
        })
    );
}

/// A screen that could not reach the engine has no service to narrow to, so the
/// capture is the whole stack it always was — no list, and no line to type a name
/// on either.
#[test]
fn a_capture_with_no_services_in_hand_is_the_whole_stack_it_always_was() {
    let (mut acting, _) = sending("backup");

    let question = showing(&acting);

    assert!(
        question.contains("Capture the configuration of the whole stack?"),
        "{question}"
    );
    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Backup { service: None })
    );
}
