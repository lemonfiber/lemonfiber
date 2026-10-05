use super::{pairings, unmatched, Connection, Unmade};
use crate::ports::service::{ApplicationKind, ClientKind, Subtitled};
use crate::seed::State;
use crate::test_support::{a_placed, an_installed};
use crate::wiring::{Chosen, Fillers};

/// The shipped stack's asks answered against `installed` with `chosen`, after
/// `changing` the stack.
fn shipped(
    installed: &[crate::plugin::Installed],
    chosen: &Chosen,
    changing: impl FnOnce(&mut lemonfiber_manifest::Manifest),
) -> Fillers {
    crate::test_support::stack()
        .manifest()
        .map(|mut manifest| {
            changing(&mut manifest);
            Fillers::of(&manifest, installed, chosen, None)
        })
        .unwrap_or_default()
}

/// A Usenet client a plugin brought, chosen to answer for the stack's own.
fn chosen_stand_in(
    api: Option<lemonfiber_manifest::Api>,
    listens: Option<u16>,
) -> (Vec<crate::plugin::Installed>, Chosen) {
    (
        vec![an_installed(
            "stand-in",
            vec![a_placed("stand-in", &["download.usenet"], api, listens)],
        )],
        Chosen::read(Some("download.usenet=stand-in")),
    )
}

/// An adapter of this kind, as a plugin's service names one.
fn speaking(kind: lemonfiber_manifest::ApiKind) -> lemonfiber_manifest::Api {
    lemonfiber_manifest::Api {
        kind,
        key_source: lemonfiber_manifest::KeySource::ConfigIni,
        path: None,
        version: Some(3),
    }
}

/// The reasons the shipped stack's Sonarr is given for each pairing that comes to
/// nothing.
fn reasons(fillers: &Fillers) -> Vec<String> {
    unmatched(fillers)
        .into_iter()
        .filter(|wiring| wiring.connection.ends_with("into Sonarr"))
        .filter_map(|wiring| match wiring.state {
            State::Unmatched { reason } => Some(reason),
            _ => None,
        })
        .collect()
}

/// Every download ask the shipped stack makes comes to a download client of the kind
/// its filler speaks.
#[test]
fn every_download_ask_the_shipped_stack_makes_comes_to_a_client() {
    let fillers = shipped(&[], &Chosen::default(), |_| ());

    let made: Vec<(String, Option<Connection>)> = pairings(&fillers)
        .iter()
        .filter(|pairing| pairing.ask.capability.starts_with("download."))
        .map(|pairing| {
            (
                pairing.filler.id.clone(),
                pairing.made.ok().map(|(connection, _)| connection),
            )
        })
        .collect();
    assert_eq!(made.len(), 6, "two asks from each of three *arrs: {made:?}");
    assert!(made.iter().all(|(id, connection)| match id.as_str() {
        "sabnzbd" => *connection == Some(Connection::DownloadClient(ClientKind::Sabnzbd)),
        _ => *connection == Some(Connection::DownloadClient(ClientKind::Qbittorrent)),
    }));
}

/// Every curator the shipped stack's curation asks reach comes to the connection its
/// media makes it, and the ones nothing connects are said — the music and book curators
/// to the two askers that deal in neither, and not the book curator to the indexer, which
/// it reaches itself through what it asks for.
#[test]
fn every_curation_ask_the_shipped_stack_makes_is_connected_or_said() {
    let fillers = shipped(&[], &Chosen::default(), |_| ());

    let made: Vec<(String, String, Option<Connection>)> = pairings(&fillers)
        .iter()
        .filter(|pairing| pairing.ask.capability == "library.curate")
        .filter_map(|pairing| {
            let connection = pairing.made.ok().map(|(connection, _)| connection);
            connection.is_some().then(|| {
                (
                    pairing.asker.id.clone(),
                    pairing.filler.id.clone(),
                    connection,
                )
            })
        })
        .collect();
    let said: Vec<String> = unmatched(&fillers)
        .into_iter()
        .map(|wiring| wiring.connection)
        .collect();

    assert_eq!(
        made,
        vec![
            (
                "prowlarr".to_owned(),
                "sonarr".to_owned(),
                Some(Connection::Application(ApplicationKind::Sonarr))
            ),
            (
                "prowlarr".to_owned(),
                "radarr".to_owned(),
                Some(Connection::Application(ApplicationKind::Radarr))
            ),
            (
                "prowlarr".to_owned(),
                "lidarr".to_owned(),
                Some(Connection::Application(ApplicationKind::Lidarr))
            ),
            (
                "seerr".to_owned(),
                "sonarr".to_owned(),
                Some(Connection::Fulfilment { television: true })
            ),
            (
                "seerr".to_owned(),
                "radarr".to_owned(),
                Some(Connection::Fulfilment { television: false })
            ),
            (
                "bazarr".to_owned(),
                "sonarr".to_owned(),
                Some(Connection::Subtitles(Subtitled::Sonarr))
            ),
            (
                "bazarr".to_owned(),
                "radarr".to_owned(),
                Some(Connection::Subtitles(Subtitled::Radarr))
            ),
        ]
    );
    assert_eq!(
        said,
        vec![
            "Lidarr into Seerr",
            "Bindery into Seerr",
            "Lidarr into Bazarr",
            "Bindery into Bazarr",
        ]
    );
}

/// A curator filing media an asker deals in none of is said, naming the media, and one
/// naming no media at all is said as that.
#[test]
fn a_curator_filing_media_nothing_hands_the_asker_is_said_naming_it() {
    let fillers = shipped(&[], &Chosen::default(), |manifest| {
        for service in &mut manifest.services {
            if service.id == "radarr" {
                service.media_types = Vec::new();
            }
        }
    });

    let seerrs: Vec<String> = unmatched(&fillers)
        .into_iter()
        .filter(|wiring| wiring.connection.ends_with("into Seerr"))
        .filter_map(|wiring| match wiring.state {
            State::Unmatched { reason } => Some(reason),
            _ => None,
        })
        .collect();

    assert!(seerrs.contains(
        &"Radarr fills library.curate, which Seerr asks for, and names no media it files, so \
          lemonfiber cannot say what to hand Seerr"
            .to_owned()
    ));
    assert!(seerrs.contains(
        &"Lidarr fills library.curate, which Seerr asks for, and files music, which lemonfiber \
          does not hand Seerr"
            .to_owned()
    ));
}

/// A filler naming no adapter is reported, naming what fills, what asked and why.
#[test]
fn a_filler_naming_no_adapter_is_reported_as_reached_by_nothing() {
    let (installed, chosen) = chosen_stand_in(None, Some(6789));
    let fillers = shipped(&installed, &chosen, |_| ());

    assert_eq!(
        unmatched(&fillers)
            .iter()
            .map(|wiring| wiring.connection.as_str())
            .filter(|connection| connection.starts_with("stand-in"))
            .collect::<Vec<&str>>(),
        vec![
            "stand-in the stand-in into Sonarr",
            "stand-in the stand-in into Radarr",
            "stand-in the stand-in into Lidarr",
        ]
    );
    assert_eq!(
        reasons(&fillers),
        vec![
            "stand-in the stand-in fills download.usenet, which Sonarr asks for, and names no \
             adapter lemonfiber could tell Sonarr about it through"
                .to_owned()
        ]
    );
}

/// A filler naming an adapter and no port it answers on is reported.
#[test]
fn a_filler_saying_no_port_is_reported_as_reached_by_nothing() {
    let (installed, chosen) =
        chosen_stand_in(Some(speaking(lemonfiber_manifest::ApiKind::Sabnzbd)), None);
    let fillers = shipped(&installed, &chosen, |_| ());

    assert_eq!(
        reasons(&fillers),
        vec![
            "stand-in the stand-in fills download.usenet, which Sonarr asks for, and does not \
             say which port it answers on inside the stack's network"
                .to_owned()
        ]
    );
}

/// A filler speaking an adapter nothing pairs with what asked is reported.
#[test]
fn a_filler_speaking_an_adapter_nothing_pairs_is_reported_as_reached_by_nothing() {
    let (installed, chosen) = chosen_stand_in(
        Some(speaking(lemonfiber_manifest::ApiKind::Servarr)),
        Some(6789),
    );
    let fillers = shipped(&installed, &chosen, |_| ());

    assert_eq!(
        reasons(&fillers),
        vec![
            "stand-in the stand-in fills download.usenet, which Sonarr asks for, and nothing \
             in lemonfiber tells Sonarr about a service that speaks stand-in the stand-in's \
             adapter"
                .to_owned()
        ]
    );
}

/// An asker naming no adapter is connected to nothing, since what it speaks decides.
#[test]
fn an_asker_naming_no_adapter_is_connected_to_nothing() {
    let fillers = shipped(&[], &Chosen::default(), |manifest| {
        for service in &mut manifest.services {
            if service.id == "sonarr" {
                service.api = None;
            }
        }
    });

    let sonarrs: Vec<Result<Connection, Unmade>> = pairings(&fillers)
        .iter()
        .filter(|pairing| pairing.ask.by == "sonarr")
        .map(|pairing| pairing.made.map(|(connection, _)| connection))
        .collect();
    assert_eq!(sonarrs, vec![Err(Unmade::Unpaired); 2]);
}

/// An asker that is not on this machine is passed over, and an ask answered elsewhere
/// is not paired here at all.
#[test]
fn an_asker_withheld_or_an_ask_answered_elsewhere_is_not_paired() {
    let fillers = shipped(&[], &Chosen::default(), |manifest| {
        manifest.services.retain(|service| service.id != "sonarr");
    });

    let paired: Vec<(&str, &str)> = pairings(&fillers)
        .iter()
        .map(|pairing| (pairing.ask.by.as_str(), pairing.ask.capability.as_str()))
        .collect();
    assert!(paired.iter().all(|(by, _)| *by != "sonarr"), "{paired:?}");
    assert!(
        paired.iter().all(|(_, capability)| {
            capability.starts_with("download.") || *capability == "library.curate"
        }),
        "{paired:?}"
    );
}

/// A curator the indexer has no application for is said once nothing connects the two,
/// and not while the curator asks the indexer for its searches.
#[test]
fn a_curator_reaching_the_indexer_itself_is_not_said_to_be_reached_by_nothing() {
    let connected = shipped(&[], &Chosen::default(), |_| ());
    let apart = shipped(&[], &Chosen::default(), |manifest| {
        manifest
            .wirings
            .retain(|wiring| wiring.asks.as_deref() != Some("indexer.search"));
    });
    let said = |fillers: &crate::wiring::Fillers| {
        unmatched(fillers)
            .into_iter()
            .any(|wiring| wiring.connection == "Bindery into Prowlarr")
    };

    assert!(!said(&connected));
    assert!(said(&apart));
}
