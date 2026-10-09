use super::{answered, restarting};
use crate::error::codes::life::RESTART_MOVED;
use crate::model::LifecycleReport;
use crate::stack::closure::Plan;
use crate::stack::compose::Action;

/// A report of a restart of `services`, as a rehearsal would read it.
fn report(services: &[&str]) -> LifecycleReport {
    LifecycleReport {
        action: "restart".to_owned(),
        plan: Plan {
            forms: vec!["media".to_owned()],
            profiles: std::collections::BTreeSet::new(),
            services: services
                .iter()
                .map(|service| (*service).to_owned())
                .collect(),
            dropped: Vec::new(),
            filtered: Vec::new(),
            footprint: crate::stack::closure::Footprint::default(),
            running: crate::stack::closure::Running::Unasked,
        },
        command: Vec::new(),
        rehearsed: true,
        status: None,
        services: Vec::new(),
        condition: None,
        stack_edits: Vec::new(),
        port_conflicts: Vec::new(),
        forwarding: None,
        switched: None,
        held: None,
        offer: None,
    }
}

#[test]
fn a_restart_answers_the_offer_its_services_name() {
    let mut rehearsed = report(&["sonarr", "radarr"]);
    let restart = Action::Restart(Vec::new());
    assert!(answered(&restart, &mut rehearsed, None).is_ok());
    let offer = rehearsed.offer.clone();
    assert_eq!(
        offer.as_deref(),
        Some(restarting(&["sonarr".to_owned(), "radarr".to_owned()]).as_str())
    );

    let mut again = report(&["sonarr", "radarr"]);
    assert!(answered(&restart, &mut again, offer.as_deref()).is_ok());
}

#[test]
fn a_restart_answering_an_offer_for_other_services_is_refused() {
    let offer = restarting(&["sonarr".to_owned()]);
    let mut now = report(&["sonarr", "radarr"]);
    let refused = answered(&Action::Restart(Vec::new()), &mut now, Some(&offer)).err();
    assert_eq!(
        refused.as_ref().map(|problem| problem.code),
        Some(RESTART_MOVED)
    );
    assert!(refused.is_some_and(|problem| problem.meaning.contains("sonarr, radarr")));
    assert_eq!(now.offer, None, "nothing is offered by a refusal");
}

#[test]
fn only_a_restart_answers_an_offer() {
    let mut started = report(&["sonarr"]);
    assert!(answered(&Action::Up, &mut started, Some("anything")).is_ok());
    assert_eq!(started.offer, None);
}
