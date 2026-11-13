use super::{pairings, unmatched, Connection, Unmade};

use crate::ports::media::Kind;
use crate::ports::service::{ApplicationKind, Protocol};
use crate::seed::State;
use crate::test_support::{a_placed, an_installed};
use crate::wiring::{Chosen, Fillers};

/// The shipped stack's asks, every part the bundle installs moved into its plugin as
/// `moving` leaves those parts, answered against them and `installed` with `chosen`, after
/// `changing` the stack.
fn moved(
    installed: &[crate::plugin::Installed],
    chosen: &Chosen,
    changing: impl FnOnce(&mut lemonfiber_manifest::Manifest),
    moving: impl FnOnce(&mut Vec<crate::plugin::Installed>),
) -> Fillers {
    let mut parts = crate::test_support::adapted::installed_parts();
    moving(&mut parts);
    parts.extend_from_slice(installed);
    lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK)
        .map(|mut manifest| {
            changing(&mut manifest);
            Fillers::of(&manifest, &parts, chosen, None)
        })
        .unwrap_or_default()
}

/// [`moved`], with every part as the bundle pins it.
fn shipped(
    installed: &[crate::plugin::Installed],
    chosen: &Chosen,
    changing: impl FnOnce(&mut lemonfiber_manifest::Manifest),
) -> Fillers {
    moved(installed, chosen, changing, |_| ())
}

/// The services of every part moved into its plugin that serve `id`: the service itself
/// and the adapter in front of it.
fn serving<'a>(
    parts: &'a mut [crate::plugin::Installed],
    id: &'a str,
) -> impl Iterator<Item = &'a mut crate::plugin::Placed> {
    parts
        .iter_mut()
        .flat_map(|part| part.services.iter_mut())
        .filter(move |placed| placed.service == id || placed.fronts.as_deref() == Some(id))
}

/// Every ask of `capability` among `parts`, naming no API it talks in, so it reaches the
/// adapter in front of whatever fills it.
fn in_no_api(parts: &mut [crate::plugin::Installed], capability: &str) {
    for ask in parts
        .iter_mut()
        .flat_map(|part| part.services.iter_mut())
        .flat_map(|placed| placed.asks.iter_mut())
        .filter(|ask| ask.capability == capability)
    {
        ask.native = None;
    }
}

/// The shipped stack's asks with every part moved in, its Usenet client's upstream as
/// `shaping` leaves it, and every ask of a Usenet client naming no API it talks in.
fn usenet_as(shaping: fn(&mut crate::plugin::Placed)) -> Fillers {
    moved(
        &[],
        &Chosen::default(),
        |_| (),
        |parts| {
            in_no_api(parts, "download.usenet");
            for upstream in parts
                .iter_mut()
                .flat_map(|part| part.services.iter_mut())
                .filter(|placed| placed.service == "sabnzbd")
            {
                shaping(upstream);
            }
        },
    )
}

/// The shipped stack's asks with every part moved in, its `service`'s upstream answering in
/// `api`, and every ask of `capability` naming no API it talks in.
fn answering_in(service: &str, capability: &str, api: &str) -> Fillers {
    moved(
        &[],
        &Chosen::default(),
        |_| (),
        |parts| {
            in_no_api(parts, capability);
            for upstream in parts
                .iter_mut()
                .flat_map(|part| part.services.iter_mut())
                .filter(|placed| placed.service == service)
            {
                upstream.native = Some(api.to_owned());
            }
        },
    )
}

/// The reasons the shipped stack's TV curator is given for each pairing that comes to
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

/// Every download ask a curator moved into its plugin makes comes to a download client of
/// the kind its filler speaks, told it over the curator's contract.
#[test]
fn every_download_ask_the_shipped_stack_makes_comes_to_a_client() {
    let fillers = shipped(&[], &Chosen::default(), |_| ());

    let made: Vec<(String, String, Option<Connection>)> = pairings(&fillers)
        .iter()
        .filter(|pairing| pairing.ask.capability.starts_with("download."))
        .map(|pairing| {
            (
                pairing.asker.id.clone(),
                pairing.filler.id.clone(),
                pairing
                    .made
                    .clone()
                    .ok()
                    .map(|(connection, _, _)| connection),
            )
        })
        .collect();
    let client = |asker: &str, filler: &str| {
        (
            asker.to_owned(),
            filler.to_owned(),
            Some(Connection::DownloadClient(Protocol(filler.to_owned()))),
        )
    };
    assert_eq!(
        made,
        vec![
            client("bindery", "sabnzbd"),
            client("bindery", "qbittorrent"),
            client("lidarr", "sabnzbd"),
            client("lidarr", "qbittorrent"),
            client("radarr", "sabnzbd"),
            client("radarr", "qbittorrent"),
            client("sonarr", "sabnzbd"),
            client("sonarr", "qbittorrent"),
        ]
    );
}

/// A curator is told about each download client in the API that client answers in, where
/// a client of the kind it asked for answers in that API.
#[test]
fn a_curator_is_told_each_download_client_in_the_api_it_answers_in() {
    for (service, capability, api) in [
        ("sabnzbd", "download.usenet", "nzbget"),
        ("qbittorrent", "download.torrent", "transmission"),
    ] {
        let fillers = answering_in(service, capability, api);

        let told: Vec<Option<Connection>> = pairings(&fillers)
            .iter()
            .filter(|pairing| pairing.asker.id == "sonarr" && pairing.filler.id == service)
            .map(|pairing| {
                pairing
                    .made
                    .clone()
                    .ok()
                    .map(|(connection, _, _)| connection)
            })
            .collect();
        assert_eq!(
            told,
            vec![Some(Connection::DownloadClient(Protocol(api.to_owned())))],
            "{service} answering in {api}"
        );
    }
}

/// A Usenet client answering in a torrent client's API, and a torrent client in a Usenet
/// client's, come to nothing, and each is said.
#[test]
fn a_download_client_answering_in_the_other_kinds_api_is_reported() {
    for (service, capability, api, named) in [
        ("sabnzbd", "download.usenet", "transmission", "SABnzbd"),
        ("qbittorrent", "download.torrent", "nzbget", "qBittorrent"),
    ] {
        let fillers = answering_in(service, capability, api);

        assert_eq!(
            reasons(&fillers),
            vec![format!(
                "{named} fills {capability}, which Sonarr asks for, and nothing in lemonfiber \
                 tells Sonarr about a service answering in {named}'s API"
            )],
            "{service} answering in {api}"
        );
    }
}

/// Every curator the shipped stack's curation asks reach comes to the connection its
/// media makes it, and the ones nothing connects are said: the music curator to the two
/// askers that deal in Servarr's API and not in music, and the music and book curators to
/// the request service. The book curator answers in no API those two ask in, so neither
/// reaches it.
#[test]
fn every_curation_ask_the_shipped_stack_makes_is_connected_or_said() {
    let fillers = shipped(&[], &Chosen::default(), |_| ());

    let made: Vec<(String, String, Option<Connection>)> = pairings(&fillers)
        .iter()
        .filter(|pairing| pairing.ask.capability == "library.curate")
        .filter_map(|pairing| {
            let connection = pairing
                .made
                .clone()
                .ok()
                .map(|(connection, _, _)| connection);
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
                "bazarr".to_owned(),
                "radarr".to_owned(),
                Some(Connection::Subtitles(Kind::Movies))
            ),
            (
                "bazarr".to_owned(),
                "sonarr".to_owned(),
                Some(Connection::Subtitles(Kind::Tv))
            ),
            (
                "prowlarr".to_owned(),
                "lidarr".to_owned(),
                Some(Connection::Application(ApplicationKind::Music))
            ),
            (
                "prowlarr".to_owned(),
                "radarr".to_owned(),
                Some(Connection::Application(ApplicationKind::Movies))
            ),
            (
                "prowlarr".to_owned(),
                "sonarr".to_owned(),
                Some(Connection::Application(ApplicationKind::Tv))
            ),
            (
                "recyclarr".to_owned(),
                "radarr".to_owned(),
                Some(Connection::Quality(Kind::Movies))
            ),
            (
                "recyclarr".to_owned(),
                "sonarr".to_owned(),
                Some(Connection::Quality(Kind::Tv))
            ),
            (
                "seerr".to_owned(),
                "radarr".to_owned(),
                Some(Connection::Fulfilment { kind: Kind::Movies })
            ),
            (
                "seerr".to_owned(),
                "sonarr".to_owned(),
                Some(Connection::Fulfilment { kind: Kind::Tv })
            ),
        ]
    );
    assert_eq!(
        said,
        vec![
            "Lidarr into Bazarr",
            "Lidarr into Recyclarr",
            "Bindery into Seerr",
            "Lidarr into Seerr",
        ]
    );
}

/// A curator filing media an asker deals in none of is said, naming the media, and one
/// naming no media at all is said as that.
#[test]
fn a_curator_filing_media_nothing_hands_the_asker_is_said_naming_it() {
    let fillers = moved(
        &[],
        &Chosen::default(),
        |_| (),
        |parts| {
            for placed in serving(parts, "radarr") {
                placed.media_types = Vec::new();
            }
        },
    );

    let request_services: Vec<String> = unmatched(&fillers)
        .into_iter()
        .filter(|wiring| wiring.connection.ends_with("into Seerr"))
        .filter_map(|wiring| match wiring.state {
            State::Unmatched { reason } => Some(reason),
            _ => None,
        })
        .collect();

    assert!(request_services.contains(
        &"Radarr fills library.curate, which Seerr asks for, and names no media it files, so \
          lemonfiber cannot say what to hand Seerr"
            .to_owned()
    ));
    assert!(request_services.contains(
        &"Lidarr fills library.curate, which Seerr asks for, and files music, which lemonfiber \
          does not hand Seerr"
            .to_owned()
    ));
}

/// A filler naming no API of its own is reported, naming what fills, what asked and why.
#[test]
fn a_filler_naming_no_api_of_its_own_is_reported_as_reached_by_nothing() {
    let fillers = usenet_as(|upstream| upstream.native = None);

    assert_eq!(
        unmatched(&fillers)
            .iter()
            .map(|wiring| wiring.connection.as_str())
            .filter(|connection| connection.starts_with("SABnzbd"))
            .collect::<Vec<&str>>(),
        vec![
            "SABnzbd into Bindery",
            "SABnzbd into Lidarr",
            "SABnzbd into Radarr",
            "SABnzbd into Sonarr",
        ]
    );
    assert_eq!(
        reasons(&fillers),
        vec![
            "SABnzbd fills download.usenet, which Sonarr asks for, and names no API of its \
             own that lemonfiber could tell Sonarr about it in"
                .to_owned()
        ]
    );
}

/// A filler naming an API of its own and no port it answers on is reported.
#[test]
fn a_filler_saying_no_port_is_reported_as_reached_by_nothing() {
    let fillers = usenet_as(|upstream| {
        upstream.listens = None;
        upstream.reached = None;
    });

    assert_eq!(
        reasons(&fillers),
        vec![
            "SABnzbd fills download.usenet, which Sonarr asks for, and does not say which \
             port it answers on inside the stack's network"
                .to_owned()
        ]
    );
}

/// A filler answering in an API nothing pairs with what asked is reported.
#[test]
fn a_filler_answering_in_an_api_nothing_pairs_is_reported_as_reached_by_nothing() {
    let fillers = usenet_as(|upstream| upstream.native = Some("servarr".to_owned()));

    assert_eq!(
        reasons(&fillers),
        vec![
            "SABnzbd fills download.usenet, which Sonarr asks for, and nothing in lemonfiber \
             tells Sonarr about a service answering in SABnzbd's API"
                .to_owned()
        ]
    );
}

/// An asker speaking no contract it is paired by is not paired here: whatever wires its
/// asks is another pass's, and nothing here reports it as reached by nothing.
#[test]
fn an_asker_speaking_no_contract_is_not_paired_here() {
    let fillers = moved(
        &[],
        &Chosen::default(),
        |_| (),
        |parts| {
            for placed in serving(parts, "sonarr") {
                placed.speaks = Vec::new();
            }
        },
    );

    assert!(pairings(&fillers)
        .iter()
        .all(|pairing| pairing.ask.by != "sonarr"));
    assert!(unmatched(&fillers)
        .iter()
        .all(|wiring| !wiring.connection.ends_with("into Sonarr")));
}

/// An asker that is not on this machine is passed over, and an ask answered elsewhere
/// is not paired here at all.
#[test]
fn an_asker_withheld_or_an_ask_answered_elsewhere_is_not_paired() {
    let fillers = moved(
        &[],
        &Chosen::default(),
        |manifest| manifest.services.retain(|service| service.id != "sonarr"),
        |parts| parts.retain(|part| part.plugin != "sonarr"),
    );

    let paired: Vec<(&str, &str)> = pairings(&fillers)
        .iter()
        .map(|pairing| (pairing.ask.by.as_str(), pairing.ask.capability.as_str()))
        .collect();
    assert!(paired.iter().all(|(by, _)| *by != "sonarr"), "{paired:?}");
    assert!(
        paired.iter().all(|(_, capability)| {
            capability.starts_with("download.")
                || *capability == "library.curate"
                || *capability == "indexer.search"
        }),
        "{paired:?}"
    );
}

/// A curator the indexer has no application for is said once nothing connects the two,
/// and not while the curator asks the indexer for its searches.
#[test]
fn a_curator_reaching_the_indexer_itself_is_not_said_to_be_reached_by_nothing() {
    let curating = |parts: &mut Vec<crate::plugin::Installed>| in_no_api(parts, "library.curate");
    let connected = moved(&[], &Chosen::default(), |_| (), curating);
    let apart = moved(
        &[],
        &Chosen::default(),
        |manifest| {
            manifest
                .wirings
                .retain(|wiring| wiring.asks.as_deref() != Some("indexer.search"));
        },
        curating,
    );
    let said = |fillers: &crate::wiring::Fillers| {
        unmatched(fillers)
            .into_iter()
            .any(|wiring| wiring.connection == "Bindery into Prowlarr")
    };

    assert!(!said(&connected));
    assert!(said(&apart));
}

/// A plugin's service asking is connected to nothing, whatever fills what it asked for —
/// one of the stack's own or another plugin's — and each pair is said with why.
#[test]
fn a_plugin_asker_is_connected_to_nothing() {
    let mut curator = a_placed("kept", &["library.curate"], Some(8990));
    curator.media_types = vec!["movies".to_owned()];
    let mut asker = a_placed("prowlarr", &[], Some(9696));
    asker.speaks = vec![lemonfiber_contract::spoken(
        lemonfiber_contract::capabilities::indexer::search::CAPABILITY,
        lemonfiber_contract::capabilities::indexer::search::MAJOR,
    )];
    asker.asks = vec![crate::plugin::Asking {
        capability: "library.curate".to_owned(),
        each: true,
        native: None,
        files: None,
        media_types: Vec::new(),
    }];
    let installed = vec![
        an_installed("asking", vec![asker]),
        an_installed("kept", vec![curator]),
    ];
    let fillers = moved(
        &installed,
        &Chosen::default(),
        |manifest| manifest.services.retain(|service| service.id != "prowlarr"),
        |parts| parts.retain(|part| part.plugin != "prowlarr"),
    );

    let made: Vec<(&str, Result<Connection, Unmade>)> = pairings(&fillers)
        .iter()
        .filter(|pairing| pairing.ask.by == "prowlarr")
        .map(|pairing| {
            (
                pairing.filler.id.as_str(),
                pairing.made.clone().map(|(connection, _, _)| connection),
            )
        })
        .collect();
    let said: Vec<String> = unmatched(&fillers)
        .into_iter()
        .filter(|wiring| wiring.connection.ends_with("into prowlarr the stand-in"))
        .filter_map(|wiring| match wiring.state {
            State::Unmatched { reason } => Some(reason),
            _ => None,
        })
        .collect();

    assert!(made.iter().any(|(id, _)| *id == "sonarr"), "{made:?}");
    assert!(made.iter().any(|(id, _)| *id == "kept"), "{made:?}");
    assert!(
        made.iter().all(|(_, made)| *made == Err(Unmade::Asked)),
        "{made:?}"
    );
    assert_eq!(said.len(), made.len(), "{said:?}");
    assert!(said.contains(
        &"Sonarr fills library.curate, which prowlarr the stand-in asks for, and prowlarr the \
          stand-in is a third-party plugin's service, which is handed another service's \
          credential only where the operator granted it in an install or update"
            .to_owned()
    ));
}

mod crossing;
