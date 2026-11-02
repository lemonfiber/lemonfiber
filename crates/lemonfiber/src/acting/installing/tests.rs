//! A start the dashboard sent that offers to install a part this build carries: the
//! question put under what is on offer, and the yes carried with the offer as answered.

use lemonfiber_core::app::{Command, InstallOffered};

use lemonfiber_core::app::Outcome;

use super::{installing, started, Press, Stage, Taken, Wanted};
use crate::acting::offer::tests::offering;
use crate::acting::Acting;
use crate::render::fixtures::a_lifecycle;

/// A start of the whole stack, as the dashboard takes it.
fn an_up() -> Taken {
    Taken {
        covers: Vec::new(),
        command: Command::Up {
            forms: Vec::new(),
            offer: None,
        },
        each: "form",
    }
}

/// What a start that answered no install it offers came to.
fn unanswered() -> lemonfiber_core::model::LifecycleReport {
    let mut report = a_lifecycle("up", crate::render::fixtures::a_plan("library", Vec::new()));
    report.status = None;
    report.offer = Some("the-offer".to_owned());
    report.installing = vec![InstallOffered {
        plugin: "jellyfin".to_owned(),
        offer: Some("its-install".to_owned()),
        why: None,
    }];
    report
}

/// A start that started nothing for want of the yes to its install is put as the
/// question under what is on offer, and a yes carries the start answering that offer.
#[test]
fn an_unanswered_install_is_asked_and_a_yes_carries_the_offer() {
    let Some(offer) = offering("up") else {
        unreachable!("the dashboard offers up");
    };

    let asked = started(offer, an_up(), &unanswered());

    let Stage::Installing {
        offer,
        taken,
        account,
    } = asked
    else {
        unreachable!("an unanswered install is a question");
    };
    let mut stage = Stage::Idle;
    let carried = installing(&mut stage, offer, taken, account, &Press::Typed('y'));
    assert!(matches!(
        carried,
        Wanted::Carry(Command::Up { offer: Some(answered), .. }) if answered == "the-offer"
    ));
    assert!(matches!(stage, Stage::Running { .. }));
}

/// Anything but a yes leaves the install unanswered and starts nothing; a start that
/// started, or a rehearsal, is read rather than asked about.
#[test]
fn anything_but_a_yes_starts_nothing_and_a_start_that_ran_is_read() {
    let Some(offer) = offering("up") else {
        unreachable!("the dashboard offers up");
    };
    let Stage::Installing {
        offer: asked,
        taken,
        account,
    } = started(offer, an_up(), &unanswered())
    else {
        unreachable!("an unanswered install is a question");
    };
    let mut stage = Stage::Idle;
    let declined = installing(&mut stage, asked, taken, account, &Press::Typed('n'));
    assert!(matches!(declined, Wanted::Nothing));
    assert!(matches!(stage, Stage::Idle));

    let mut ran = unanswered();
    ran.command = vec!["docker".to_owned()];
    assert!(matches!(started(offer, an_up(), &ran), Stage::Came(_)));
    let mut rehearsed = unanswered();
    rehearsed.rehearsed = true;
    assert!(matches!(
        started(offer, an_up(), &rehearsed),
        Stage::Came(_)
    ));
}

/// Everything the dashboard's box says, title first, as one piece of text.
fn showing(acting: &Acting) -> String {
    acting.pane(40, 120).map_or_else(String::new, |pane| {
        let mut said = vec![pane.title];
        said.extend(pane.lines.iter().map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        }));
        said.join("\n")
    })
}

/// On the dashboard, a start that came back unanswered opens the question under what is
/// on offer; moving through it keeps the question open, and a yes sends the start
/// answering the offer.
#[test]
fn the_dashboard_asks_about_the_install_and_a_yes_sends_the_answer() {
    let Some(offer) = offering("up") else {
        unreachable!("the dashboard offers up");
    };
    let mut acting = Acting::opened();
    acting.stage = Stage::Running {
        offer,
        taken: an_up(),
    };

    acting.came_to(Ok(Outcome::Lifecycle(unanswered())));

    let asked = showing(&acting);
    assert!(
        asked.contains("Install from this build, offline, and start")
            && asked.contains("before anything else of the form is written or started"),
        "{asked}"
    );
    assert_eq!(acting.pressed(&Press::Forward), Wanted::Nothing);
    let moved = showing(&acting);
    assert!(
        moved.contains("more line above")
            && moved.contains("Install from this build, offline, and start"),
        "moving through what is on offer keeps the question open: {moved}"
    );
    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Up {
            forms: Vec::new(),
            offer: Some("the-offer".to_owned()),
        })
    );
}

/// A switch that came back unanswered is asked about as a start is, and the yes carries
/// the switch answering the offer.
#[test]
fn an_unanswered_switch_is_asked_and_a_yes_carries_the_offer() {
    let Some(offer) = offering("switch") else {
        unreachable!("the dashboard offers switch");
    };
    let switching = Taken {
        command: Command::Switch {
            forms: vec!["library".to_owned()],
            offer: None,
        },
        ..an_up()
    };

    let Stage::Installing {
        offer,
        taken,
        account,
    } = started(offer, switching, &unanswered())
    else {
        unreachable!("an unanswered switch is a question");
    };
    let mut stage = Stage::Idle;

    assert_eq!(
        installing(&mut stage, offer, taken, account, &Press::Typed('Y')),
        Wanted::Carry(Command::Switch {
            forms: vec!["library".to_owned()],
            offer: Some("the-offer".to_owned()),
        })
    );
}
