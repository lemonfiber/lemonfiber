use super::{Address, Fillers};
use crate::origin::Origin;
use crate::test_support::{a_placed, an_installed};
use crate::wiring::Chosen;

/// Where the stack these read is written to disk.
fn project() -> &'static std::path::Path {
    std::path::Path::new("/opt/lemonfiber/stack")
}

/// The shipped stack's asks, answered against `installed` with `chosen`, with the stack's
/// services changed by `changing` first.
fn shipped(
    installed: &[crate::plugin::Installed],
    chosen: &Chosen,
    changing: impl FnOnce(&mut lemonfiber_manifest::Manifest),
) -> Fillers {
    crate::test_support::stack()
        .manifest()
        .map(|mut manifest| {
            changing(&mut manifest);
            Fillers::of(&manifest, installed, chosen, Some(project()))
        })
        .unwrap_or_default()
}

/// A Usenet client a plugin brought, reading its key from beneath its own directory.
fn usenet_stand_in(listens: Option<u16>) -> crate::plugin::Placed {
    a_placed(
        "nzbget",
        &["download.usenet"],
        Some(lemonfiber_manifest::Api {
            kind: lemonfiber_manifest::ApiKind::Sabnzbd,
            key_source: lemonfiber_manifest::KeySource::ConfigIni,
            path: Some("/config/nzbget.conf".to_owned()),
            version: None,
        }),
        listens,
    )
}

/// Each of the stack's services is reached at its own id on the port it says it listens
/// on, not the one it publishes — and the torrent client at the tunnel whose network it
/// shares.
#[test]
fn a_stack_service_is_reached_where_it_says_it_listens() {
    let fillers = shipped(&[], &Chosen::default(), |_| ());

    let usenet = fillers.service("sabnzbd");
    assert_eq!(
        usenet.and_then(|one| one.address.clone()),
        Some(Address {
            host: "sabnzbd".to_owned(),
            port: 8080
        })
    );
    assert_eq!(usenet.and_then(|one| one.published), Some(8085));
    assert_eq!(
        usenet.and_then(|one| one.key_file.clone()),
        Some(project().join("config/sabnzbd/sabnzbd.ini"))
    );
    assert_eq!(usenet.map(|one| one.origin.clone()), Some(Origin::Bundled));
    assert_eq!(
        fillers
            .service("qbittorrent")
            .and_then(|one| one.address.clone()),
        Some(Address {
            host: "gluetun".to_owned(),
            port: 8081
        })
    );
    assert_eq!(
        fillers.service("gluetun").map(|one| one.address.clone()),
        Some(None),
        "a service that says nowhere it listens has no address"
    );
}

/// A service waiting on another that is no tunnel is reached at its own id.
#[test]
fn a_service_waiting_on_something_other_than_a_tunnel_is_reached_at_its_own_id() {
    let fillers = shipped(&[], &Chosen::default(), |manifest| {
        for service in &mut manifest.services {
            if service.id == "sabnzbd" {
                service.depends_on = vec!["prowlarr".to_owned()];
            }
        }
    });

    assert_eq!(
        fillers
            .service("sabnzbd")
            .and_then(|one| one.address.as_ref())
            .map(|at| at.host.as_str()),
        Some("sabnzbd")
    );
}

/// Every ask the stack declares is answered, and a link kept to a named service is not
/// an ask.
#[test]
fn every_ask_the_stack_declares_is_answered_and_no_link_by_name() {
    let mut asks = 0;
    let fillers = shipped(&[], &Chosen::default(), |manifest| {
        asks = manifest
            .wirings
            .iter()
            .filter(|wiring| wiring.asks.is_some())
            .count();
    });

    assert!(asks > 0, "the shipped stack asks for something");
    assert_eq!(fillers.asks().len(), asks);
    let usenet: Vec<&str> = fillers
        .asks()
        .iter()
        .filter(|ask| ask.by == "sonarr" && ask.capability == "download.usenet")
        .flat_map(|ask| ask.fillers.iter().map(|one| one.id.as_str()))
        .collect();
    assert_eq!(usenet, vec!["sabnzbd"]);
}

/// A plugin's service is reached at its own id, carries the plugin it came from, and
/// reads its credential from beneath its own directory — and it answers an ask it was
/// chosen for.
#[test]
fn a_plugins_service_is_reached_at_its_own_id_and_answers_where_chosen() {
    let installed = [an_installed("nzbget", vec![usenet_stand_in(Some(6789))])];
    let fillers = shipped(
        &installed,
        &Chosen::read(Some("download.usenet=nzbget")),
        |_| (),
    );

    let stand_in = fillers.service("nzbget");
    assert_eq!(
        stand_in.and_then(|one| one.address.clone()),
        Some(Address {
            host: "nzbget".to_owned(),
            port: 6789
        })
    );
    assert_eq!(
        stand_in.map(|one| one.origin.clone()),
        Some(Origin::Plugin {
            named: "nzbget".to_owned()
        })
    );
    assert_eq!(
        stand_in.and_then(|one| one.key_file.clone()),
        Some(project().join("config/nzbget/nzbget.conf"))
    );
    let answering: Vec<&str> = fillers
        .asks()
        .iter()
        .filter(|ask| ask.capability == "download.usenet")
        .flat_map(|ask| ask.fillers.iter().map(|one| one.id.as_str()))
        .collect();
    assert_eq!(answering, vec!["nzbget"; 3]);
}

/// A plugin's service recorded before names were kept is called by its id, and one that
/// says nowhere it listens has no address.
#[test]
fn a_plugins_service_with_no_name_or_port_is_called_by_its_id_and_reached_nowhere() {
    let mut unnamed = usenet_stand_in(None);
    unnamed.name = String::new();
    let fillers = shipped(
        &[an_installed("nzbget", vec![unnamed])],
        &Chosen::default(),
        |_| (),
    );

    let stand_in = fillers.service("nzbget");
    assert_eq!(stand_in.map(|one| one.name.as_str()), Some("nzbget"));
    assert_eq!(stand_in.map(|one| one.address.clone()), Some(None));
}

/// What lemonfiber speaks to through an adapter is every service naming it, the
/// stack's first.
#[test]
fn every_service_speaking_an_adapter_is_found_the_stacks_first() {
    let installed = [an_installed("nzbget", vec![usenet_stand_in(Some(6789))])];
    let fillers = shipped(&installed, &Chosen::default(), |_| ());

    let speaking: Vec<&str> = fillers
        .speaking(lemonfiber_manifest::ApiKind::Sabnzbd)
        .map(|one| one.id.as_str())
        .collect();
    assert_eq!(speaking, vec!["sabnzbd", "nzbget"]);
}

/// Without a project there is no file beneath it to read a credential from.
#[test]
fn without_a_project_no_service_has_a_credential_file() {
    let installed = [an_installed("nzbget", vec![usenet_stand_in(Some(6789))])];
    let fillers = crate::test_support::stack()
        .manifest()
        .map(|manifest| Fillers::of(&manifest, &installed, &Chosen::default(), None))
        .unwrap_or_default();

    assert_eq!(
        fillers.service("sabnzbd").map(|one| one.key_file.clone()),
        Some(None)
    );
    assert_eq!(
        fillers.service("nzbget").map(|one| one.key_file.clone()),
        Some(None)
    );
}

/// A stack service's credential is kept under its own id; a plugin's in a namespace of
/// its own, so a plugin cannot name its way onto the stack's.
#[test]
fn a_plugins_credential_is_kept_apart_from_the_stacks() {
    let installed = [an_installed(
        "namesake",
        vec![a_placed("qbittorrent-two", &[], None, None)],
    )];
    let fillers = shipped(&installed, &Chosen::default(), |_| ());
    let setting = |id: &str| {
        fillers
            .service(id)
            .and_then(|one| fillers.setting(one, crate::config::PASSWORD_SUFFIX))
    };

    assert_eq!(
        setting("qbittorrent").as_deref(),
        Some("QBITTORRENT_PASSWORD")
    );
    assert_eq!(
        setting("qbittorrent-two").as_deref(),
        Some("PLUGIN_QBITTORRENT__TWO_PASSWORD")
    );
}

/// Two plugins' services whose ids run into each other's endings never share a
/// setting: `a-api` holding a key and `a` holding its API key are kept apart.
#[test]
fn a_plugin_id_running_into_anothers_ending_is_kept_apart() {
    let installed = [
        an_installed("first", vec![a_placed("a", &[], None, None)]),
        an_installed("second", vec![a_placed("a-api", &[], None, None)]),
    ];
    let fillers = shipped(&installed, &Chosen::default(), |_| ());
    let spelled = |id: &str, holds: &str| {
        fillers
            .service(id)
            .map(|one| super::spelled(one, holds))
            .unwrap_or_default()
    };

    assert_eq!(spelled("a", "_API_KEY"), "PLUGIN_A_API_KEY");
    assert_eq!(spelled("a-api", "_KEY"), "PLUGIN_A__API_KEY");
    assert_ne!(spelled("a", "_API_KEY"), spelled("a-api", "_KEY"));
}

/// Every ending a credential takes is one `_` and then a letter, which is what lets a
/// plugin's setting be read back into the id and the ending it was made of.
#[test]
fn every_credential_ending_is_one_underscore_and_then_a_letter() {
    for suffix in crate::config::CREDENTIAL_SUFFIXES {
        let mut chars = suffix.chars();
        assert_eq!(chars.next(), Some('_'), "{suffix}");
        assert!(
            chars.next().is_some_and(|c| c.is_ascii_alphabetic()),
            "{suffix}"
        );
    }
}

/// A plugin's setting is refused where another service here takes the same name for any
/// credential, where it would end in anything but a credential's ending, and never for
/// the stack's own.
#[test]
fn a_plugin_setting_another_service_takes_is_refused() {
    // Written alike once case and `-` or `_` are set aside, as a record kept before
    // that was refused at install may still be.
    let installed = [
        an_installed("first", vec![a_placed("kept-one", &[], None, None)]),
        an_installed("second", vec![a_placed("kept_one", &[], None, None)]),
        an_installed("third", vec![a_placed("apart", &[], None, None)]),
    ];
    let fillers = shipped(&installed, &Chosen::default(), |_| ());
    let setting = |id: &str, holds: &str| {
        fillers
            .service(id)
            .and_then(|one| fillers.setting(one, holds))
    };

    assert_eq!(setting("kept-one", crate::config::PASSWORD_SUFFIX), None);
    assert_eq!(setting("kept_one", crate::config::PASSWORD_SUFFIX), None);
    assert_eq!(setting("apart", "_KEY"), None);
    assert_eq!(
        setting("apart", crate::config::PASSWORD_SUFFIX).as_deref(),
        Some("PLUGIN_APART_PASSWORD")
    );
    assert_eq!(setting("sonarr", "_KEY").as_deref(), Some("SONARR_KEY"));
}
