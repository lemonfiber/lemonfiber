use super::{container, manifest};
use crate::config::Protocols;
use crate::docker::{read, survey, undeclared, Halted, State};
use crate::ports::docker::{Health, Lifecycle};

/// A service lemonfiber stopped reads as stopped whatever code it left with. A tunnel that
/// exits 1 on the signal, or anything killed when its grace period ran out, was still
/// stopped by the operator, and reporting that as a fault is what an operator who just
/// pressed stop would be shown.
#[test]
fn a_container_lemonfiber_stopped_reads_as_stopped_whatever_it_exited_with() {
    let halted = Halted::new(["id-gluetun".to_owned(), "id-sonarr".to_owned()]);
    for (service, lifecycle, code) in [
        ("gluetun", Lifecycle::Exited, 1),
        ("sonarr", Lifecycle::Exited, 137),
        ("sonarr", Lifecycle::Dead, 143),
    ] {
        let mut stopped = container(service, lifecycle, Health::None);
        stopped.exit = Some(code);
        assert_eq!(read(&stopped, &halted), State::Stopped, "{service} {code}");
    }

    let mut fell = container("radarr", Lifecycle::Exited, Health::None);
    fell.exit = Some(1);
    assert_eq!(
        read(&fell, &halted),
        State::Failed,
        "a container lemonfiber did not stop is read by its own code"
    );

    let mut restarting = container("gluetun", Lifecycle::Restarting, Health::None);
    restarting.exit = Some(1);
    assert_eq!(
        read(&restarting, &halted),
        State::CrashLooping,
        "a stop lemonfiber made says nothing about a container that is running again"
    );
}

/// The survey and the listing of what the stack never declared read the same record, so
/// the operator's own stop cannot read as stopped in one place and failed in another.
#[test]
fn every_listing_reads_what_lemonfiber_stopped() {
    let mut gluetun = container("gluetun", Lifecycle::Exited, Health::None);
    gluetun.exit = Some(1);
    let mut stranger = container("extra", Lifecycle::Exited, Health::None);
    stranger.exit = Some(1);
    let containers = vec![gluetun, stranger];
    let halted = Halted::new(["id-gluetun".to_owned(), "id-extra".to_owned()]);

    let profiles = vec!["torrent".to_owned()];
    let surveyed = manifest()
        .map(|manifest| {
            survey(
                &manifest,
                &profiles,
                &containers,
                &halted,
                Protocols::both(),
            )
        })
        .unwrap_or_default();
    let state = surveyed
        .iter()
        .find(|service| service.id == "gluetun")
        .map(|service| (service.state, service.exit));
    assert_eq!(state, Some((State::Stopped, Some(1))), "the code is kept");

    let strangers = manifest()
        .map(|manifest| undeclared(&manifest, &containers, &halted))
        .unwrap_or_default();
    assert_eq!(
        strangers.iter().map(|one| one.state).collect::<Vec<_>>(),
        vec![State::Stopped]
    );
}
