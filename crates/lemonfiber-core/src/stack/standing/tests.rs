use super::{brought, left_out, needed_by, standings, Standing};
use crate::config::Protocols;
use crate::docker::{Criticality, Service, State};
use crate::stack::closure::{Dropped, Protocol};
use lemonfiber_manifest::Manifest;

const STACK: &str = include_str!("../../../../../assets/media-stack/stack.toml");

/// The stack this repository ships, which is what every form here is declared by.
fn stack() -> Option<Manifest> {
    Manifest::from_toml(STACK).ok()
}

/// A service that is up and answering.
fn healthy(id: &str) -> Service {
    Service {
        id: id.to_owned(),
        name: id.to_owned(),
        describes: format!("what {id} is for"),
        profile: "search".to_owned(),
        forms: Vec::new(),
        state: State::Healthy,
        criticality: Criticality::Core,
        depends_on: Vec::new(),
        exit: None,
    }
}

/// Where the named form stands, given what is up.
fn stands(form: &str, protocols: Protocols, running: &[Service]) -> Option<Standing> {
    let manifest = stack()?;
    standings(&manifest, protocols, running)
        .into_iter()
        .find(|(id, _)| id == form)
        .map(|(_, standing)| standing)
}

/// The services a form holds, so a test can say "all of these are up".
fn services_of(form: &str, protocols: Protocols) -> Vec<Service> {
    stack()
        .and_then(|manifest| {
            crate::stack::closure::resolve(&manifest, &[form.to_owned()], protocols).ok()
        })
        .map(|plan| plan.services.iter().map(|id| healthy(id)).collect())
        .unwrap_or_default()
}

/// What a script reading this over `--json` would receive.
fn wire(standing: &Standing) -> Option<String> {
    serde_json::to_string(standing).ok()
}

#[test]
fn a_form_nothing_is_running_for_is_available_rather_than_active() {
    assert_eq!(
        stands("library", Protocols::none(), &[]),
        Some(Standing::Available),
        "serving what you already have needs no provider, and nothing is up"
    );
}

/// The state that exists because an operator seeing fewer services than they
/// expected is owed the reason at the moment they notice.
#[test]
fn a_form_the_configuration_narrows_is_partially_available_and_says_what_it_lost() {
    let usenet_only = Protocols {
        usenet: true,
        torrent: false,
    };
    let standing = stands("dl", usenet_only, &[]);
    assert!(
        matches!(&standing, Some(Standing::PartiallyAvailable { dropped })
            if dropped.iter().any(|out| out.profile == "torrent")),
        "{standing:?}"
    );
}

#[test]
fn a_form_with_nothing_left_to_run_is_unavailable() {
    assert_eq!(
        stands("dl", Protocols::none(), &[]),
        Some(Standing::Unavailable),
        "both ways of downloading are unconfigured, so there is nothing to start"
    );
}

#[test]
fn a_form_whose_every_service_is_up_is_active() {
    let running = services_of("library", Protocols::none());
    assert_eq!(
        stands("library", Protocols::none(), &running),
        Some(Standing::Active)
    );
}

/// The distinction that keeps a listing readable: an operator who started `full`
/// should not be told that eight other forms are also running.
#[test]
fn a_form_running_under_a_broader_one_is_superseded_by_it() {
    let running = services_of("full", Protocols::both());
    let standing = stands("search", Protocols::both(), &running);
    assert!(
        matches!(&standing, Some(Standing::Superseded { by }) if by == "full"),
        "search is inside full, so full is what is running: {standing:?}"
    );
    assert_eq!(
        stands("full", Protocols::both(), &running),
        Some(Standing::Active),
        "and the broadest one is active rather than superseded by anything"
    );
}

/// Half a form up is not the form up. Reported through the same condition the
/// status report uses, so the two cannot disagree about what "up" means.
#[test]
fn a_form_only_partly_up_is_not_active() {
    let mut running = services_of("library", Protocols::none());
    running.truncate(1);
    assert_eq!(
        stands("library", Protocols::none(), &running),
        Some(Standing::Available),
        "one service of four is not the form running"
    );
}

#[test]
fn every_form_the_stack_declares_gets_exactly_one_standing() {
    let manifest = stack();
    let counted = manifest.as_ref().map(|manifest| {
        (
            manifest.forms.len(),
            standings(manifest, Protocols::both(), &[]).len(),
        )
    });
    assert!(
        counted.is_some_and(|(forms, standings)| forms == standings && forms > 1),
        "{counted:?}"
    );
}

/// Pinned rather than left to the derive, because this is a published shape: a
/// script matching on `state` should keep working across a rename here.
#[test]
fn every_standing_publishes_the_state_under_one_key() {
    assert_eq!(
        wire(&Standing::Available).as_deref(),
        Some(r#"{"state":"available"}"#)
    );
    assert_eq!(
        wire(&Standing::Unavailable).as_deref(),
        Some(r#"{"state":"unavailable"}"#)
    );
    assert_eq!(
        wire(&Standing::Active).as_deref(),
        Some(r#"{"state":"active"}"#)
    );
    assert_eq!(
        wire(&Standing::Superseded {
            by: "full".to_owned()
        })
        .as_deref(),
        Some(r#"{"state":"superseded","by":"full"}"#)
    );
    assert_eq!(
        wire(&Standing::PartiallyAvailable {
            dropped: vec![Dropped {
                profile: "torrent".to_owned(),
                needs: Protocol::Torrent,
            }],
        })
        .as_deref(),
        Some(
            r#"{"state":"partially-available","dropped":[{"profile":"torrent","needs":"torrent"}]}"#
        ),
        "and the reason travels with the state, rather than needing a second call"
    );
}

/// The case superseding does not cover. Two forms overlap without either being
/// inside the other, and the shared service is what makes stopping one of them
/// somebody else's problem.
#[test]
fn a_form_that_shares_a_service_with_a_running_one_is_named() {
    let running = services_of("full", Protocols::both());
    let stack = stack();
    let told = stack
        .as_ref()
        .map(|manifest| needed_by(manifest, Protocols::both(), &running, &["tv".to_owned()]));

    assert!(
        told.as_ref()
            .is_some_and(|forms| forms.iter().any(|form| form == "full")),
        "`full` is up and holds what stopping `tv` would take away: {told:?}"
    );
    assert!(
        told.as_ref()
            .is_some_and(|forms| !forms.contains(&"tv".to_owned())),
        "and the form being stopped is not told it needs itself: {told:?}"
    );
}

/// Nobody is put out by losing a service they were not running.
#[test]
fn a_form_nobody_started_is_not_spoken_for() {
    let stack = stack();
    let told = stack
        .as_ref()
        .map(|manifest| needed_by(manifest, Protocols::both(), &[], &["tv".to_owned()]));

    assert_eq!(
        told,
        Some(Vec::new()),
        "nothing is up, so stopping anything deprives nobody: {told:?}"
    );
}

/// A form whose services are entirely its own takes nothing from anyone.
#[test]
fn stopping_the_only_form_that_is_up_is_nobody_elses_business() {
    let running = services_of("library", Protocols::none());
    let stack = stack();
    let told = stack.as_ref().map(|manifest| {
        needed_by(
            manifest,
            Protocols::none(),
            &running,
            &["library".to_owned()],
        )
    });

    assert_eq!(told, Some(Vec::new()), "{told:?}");
}

/// An unresolvable name is refused by whoever was asked, with a better sentence
/// than a list of forms that need it could give.
#[test]
fn a_form_that_does_not_resolve_deprives_nobody() {
    let running = services_of("full", Protocols::both());
    let stack = stack();
    let told = stack.as_ref().map(|manifest| {
        needed_by(
            manifest,
            Protocols::both(),
            &running,
            &["not-a-form".to_owned()],
        )
    });

    assert_eq!(told, Some(Vec::new()), "{told:?}");
}

/// The forms counted as bringing what is running, by name.
fn brought_by(protocols: Protocols, running: &[Service]) -> Vec<String> {
    stack()
        .map(|manifest| brought(&manifest, protocols, running))
        .unwrap_or_default()
        .into_iter()
        .map(|(form, _)| form)
        .collect()
}

const USENET_ONLY: Protocols = Protocols {
    usenet: true,
    torrent: false,
};

#[test]
fn nothing_running_was_brought_by_anything() {
    assert!(brought_by(Protocols::both(), &[]).is_empty());
}

/// Two forms sharing their indexers and downloaders are both named, and the
/// forms inside them are not: `hunt` is up because `tv` and `movies` are.
#[test]
fn every_form_up_in_its_own_right_is_named_and_none_inside_one() {
    let mut running = services_of("tv", Protocols::both());
    running.extend(services_of("movies", Protocols::both()));
    assert_eq!(
        brought_by(Protocols::both(), &running),
        vec!["tv", "movies"]
    );
}

/// A service that fell over is still part of the form somebody started; its
/// failure is its state, not a reason to forget why it is there.
#[test]
fn a_form_with_a_failed_service_still_brought_the_rest() {
    let mut running = services_of("library", Protocols::none());
    if let Some(first) = running.first_mut() {
        first.state = State::Failed;
    }
    assert_eq!(brought_by(Protocols::none(), &running), vec!["library"]);
}

#[test]
fn a_form_with_a_stopped_service_brought_nothing() {
    let mut running = services_of("library", Protocols::none());
    if let Some(first) = running.first_mut() {
        first.state = State::Stopped;
    }
    assert!(brought_by(Protocols::none(), &running).is_empty());
}

/// What a Usenet-only machine running `tv` and `movies` left out: the tunnel and
/// the torrent client, each once, each naming both forms and what it needed.
#[test]
fn what_the_forms_left_out_is_named_once_with_every_form_that_asked() {
    let mut running = services_of("tv", USENET_ONLY);
    running.extend(services_of("movies", USENET_ONLY));
    let filtered = stack()
        .map(|manifest| left_out(&manifest, &brought(&manifest, USENET_ONLY, &running)))
        .unwrap_or_default();
    let ids: Vec<&str> = filtered.iter().map(|out| out.id.as_str()).collect();
    assert_eq!(ids, vec!["gluetun", "qbittorrent"]);
    assert!(filtered.iter().all(|out| out.needs == Protocol::Torrent
        && out.profile == "torrent"
        && out.forms == ["tv", "movies"]));
}
