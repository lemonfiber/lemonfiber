use super::{made, pairings, unmatched, Connection, Unmade};
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

/// The shipped stack's asks with every part moved in, its Usenet client's upstream as
/// `shaping` leaves it.
fn usenet_as(shaping: fn(&mut crate::plugin::Placed)) -> Fillers {
    moved(
        &[],
        &Chosen::default(),
        |_| (),
        |parts| {
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

/// Every curator the shipped stack's curation asks reach comes to the connection its
/// media makes it, and the ones nothing connects are said — the download clients to the
/// book curator, the music and book curators to the two askers that deal in neither, and
/// not the book curator to the indexer, which it reaches itself through what it asks for.
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
            "Bindery into Bazarr",
            "Lidarr into Bazarr",
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

/// A service of `origin` answering in `kind`, filing television, reached at its id.
fn a_service(
    id: &str,
    origin: &crate::origin::Origin,
    kind: lemonfiber_manifest::ApiKind,
) -> crate::wiring::Filler {
    crate::wiring::Filler {
        minted: None,
        granted: Vec::new(),
        id: id.to_owned(),
        name: id.to_owned(),
        origin: origin.clone(),
        address: Some(crate::wiring::Address {
            host: id.to_owned(),
            port: 8000,
        }),
        published: Some(8000),
        confined_to: None,
        media_types: vec!["tv".to_owned()],
        provides: Vec::new(),
        contracts: Vec::new(),
        first_party: false,
        majors: Vec::new(),
        fronts: None,
        upstream: std::collections::BTreeMap::new(),
        native: Some(kind.name().to_owned()),
        replaces: None,
        via: None,
    }
}

/// One kind of connection the gate is held to: the contract the asker speaks and the
/// media it files, what it asks for, what fills it, and whether the connection also hands
/// the filler the asker's own key.
type Crossing = (
    (&'static str, u32),
    &'static str,
    &'static str,
    lemonfiber_manifest::ApiKind,
    bool,
);

/// Every kind of connection the table makes, once each.
fn crossing_kinds() -> [Crossing; 6] {
    use lemonfiber_contract::capabilities::indexer::{pull, search};
    use lemonfiber_contract::capabilities::library::curate;
    use lemonfiber_contract::capabilities::request::intake;
    use lemonfiber_contract::capabilities::subtitles::fetch;
    use lemonfiber_manifest::ApiKind;
    let curating = (curate::CAPABILITY, curate::MAJOR);
    [
        (curating, "tv", "download.usenet", ApiKind::Sabnzbd, false),
        (
            curating,
            "tv",
            "download.torrent",
            ApiKind::Qbittorrent,
            false,
        ),
        (
            (search::CAPABILITY, search::MAJOR),
            "tv",
            "library.curate",
            ApiKind::Servarr,
            true,
        ),
        (
            (intake::CAPABILITY, intake::MAJOR),
            "tv",
            "library.curate",
            ApiKind::Servarr,
            false,
        ),
        (
            (fetch::CAPABILITY, fetch::MAJOR),
            "tv",
            "library.curate",
            ApiKind::Servarr,
            false,
        ),
        (
            (pull::CAPABILITY, pull::MAJOR),
            "books",
            "indexer.search",
            ApiKind::Servarr,
            false,
        ),
    ]
}

/// A service of `origin` asking over `contract`, filing television, reached at its id.
fn an_asker(
    id: &str,
    origin: &crate::origin::Origin,
    contract: (&str, u32),
) -> crate::wiring::Filler {
    crate::wiring::Filler {
        native: None,
        contracts: vec![lemonfiber_contract::spoken(contract.0, contract.1)],
        ..a_service(id, origin, lemonfiber_manifest::ApiKind::Servarr)
    }
}

/// **Every credential crosses one gate, whichever connection carries it.** Each kind of
/// connection the table makes, between an asker and a filler of each pair of origins:
/// the stack's own to the stack's own, the stack's to a plugin's, a plugin's to its own
/// plugin's, a plugin's to another plugin's, and a plugin's to the stack's. A credential
/// reaches only the stack's own or the owner's own plugin, in both directions — the
/// indexer's sync hands the filler the asker's key as well as the other way round.
#[test]
fn every_connection_hands_a_credential_only_where_the_gate_lets_it_cross() {
    use crate::origin::Origin;

    let stack = Origin::Bundled;
    let one = Origin::Plugin {
        named: "one".to_owned(),
    };
    let other = Origin::Plugin {
        named: "other".to_owned(),
    };
    let kinds = crossing_kinds();
    // Whose the filler is, whose the asker is, and what the pairing comes to: made, or
    // refused because the filler's credential may not reach the asker, or because the
    // asker's may not reach the filler.
    let rows = [
        (&stack, &stack, None, None),
        (&stack, &one, Some(Unmade::Asked), Some(Unmade::Asked)),
        (&one, &one, None, None),
        (&one, &other, Some(Unmade::Asked), Some(Unmade::Asked)),
        (&one, &stack, None, Some(Unmade::Withheld)),
    ];

    let mut seen = std::collections::BTreeSet::new();
    for (speaks, files, capability, fills, both_ways) in kinds {
        for (filler_is, asker_is, refused, refused_both_ways) in &rows {
            let mut asker = an_asker("asker", asker_is, speaks);
            asker.media_types = vec![files.to_owned()];
            let filler = a_service("filler", filler_is, fills);
            let ask = crate::wiring::Ask {
                by: asker.id.clone(),
                capability: capability.to_owned(),
                fillers: vec![filler.clone()],
            };
            let fillers = crate::wiring::Fillers::holding(vec![asker.clone(), filler.clone()]);
            let came = made(&fillers, (&asker, &asker, speaks.0), &ask, &filler);
            let expected = if both_ways {
                refused_both_ways
            } else {
                refused
            };

            let connection = came.map(|(connection, ..)| connection);
            if let Some(why) = expected {
                assert_eq!(
                    connection,
                    Err(*why),
                    "{capability}: {filler_is:?}'s filler into {asker_is:?}'s asker"
                );
            } else {
                assert!(
                    connection.is_ok(),
                    "{capability}: {filler_is:?}'s filler into {asker_is:?}'s asker came to \
                     {connection:?}"
                );
                if let Ok(connection) = connection {
                    seen.insert(format!("{connection:?}"));
                }
            }
        }
    }
    assert_eq!(
        seen.len(),
        kinds.len(),
        "a connection kind was never made: {seen:?}"
    );
}

#[test]
fn a_credential_the_operator_granted_a_third_party_plugin_opens_its_connection() {
    use crate::origin::Origin;
    use lemonfiber_manifest::ApiKind;

    let stranger = Origin::Plugin {
        named: "comics".to_owned(),
    };
    let filler = crate::wiring::Filler {
        provides: vec!["library.curate".to_owned()],
        media_types: vec!["tv".to_owned()],
        ..a_service("sonarr", &Origin::Bundled, ApiKind::Servarr)
    };
    let asker = an_asker(
        "finder",
        &stranger,
        (
            lemonfiber_contract::capabilities::subtitles::fetch::CAPABILITY,
            lemonfiber_contract::capabilities::subtitles::fetch::MAJOR,
        ),
    );
    let ask = crate::wiring::Ask {
        by: asker.id.clone(),
        capability: "library.curate".to_owned(),
        fillers: vec![filler.clone()],
    };
    let ungranted = crate::wiring::Fillers::holding(vec![asker.clone(), filler.clone()]);
    let subtitles = lemonfiber_contract::capabilities::subtitles::fetch::CAPABILITY;
    let came = made(&ungranted, (&asker, &asker, subtitles), &ask, &filler).map(|(made, ..)| made);
    assert_eq!(came, Err(Unmade::Asked));

    let credential = ungranted
        .setting(&filler, crate::config::API_KEY_SUFFIX)
        .unwrap_or_default();
    let granted = crate::wiring::Filler {
        granted: vec![credential],
        ..asker
    };
    let fillers = crate::wiring::Fillers::holding(vec![granted.clone(), filler.clone()]);
    let came =
        made(&fillers, (&granted, &granted, subtitles), &ask, &filler).map(|(made, ..)| made);
    assert!(came.is_ok(), "{came:?}");
}
