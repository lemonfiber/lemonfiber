//! Every credential a connection carries crosses one gate, whoever the asker and the
//! filler belong to.

use super::super::{made, Connection, Unmade};
use lemonfiber_contract::capabilities::library::curate;
use lemonfiber_contract::capabilities::request::intake;

use crate::ports::media::Kind;

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
        contract: None,
        confined_to: None,
        config_at: None,
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
            let filler = crate::wiring::Filler {
                contracts: vec![lemonfiber_contract::spoken(
                    curate::CAPABILITY,
                    curate::MAJOR,
                )],
                media_types: vec!["tv".to_owned()],
                ..a_service("filler", filler_is, fills)
            };
            let ask = crate::wiring::Ask {
                by: asker.id.clone(),
                capability: capability.to_owned(),
                native: None,
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

/// **A third-party curator reaches a first-party request service.** The request service
/// is handed a token of the gate's and nothing of the curator's, so the curator's own key
/// crossing to it is not what the pairing turns on. A third-party request service is handed
/// no gate token for a curator not its own, and a curator speaking no contract is said.
#[test]
fn a_curator_of_any_plugin_reaches_a_request_service_the_stacks_token_may_reach() {
    use crate::origin::Origin;
    use lemonfiber_manifest::ApiKind;

    let stranger = Origin::Plugin {
        named: "stranger".to_owned(),
    };
    let moved = Origin::Plugin {
        named: "moved".to_owned(),
    };
    let curating = |speaks: bool| crate::wiring::Filler {
        contracts: if speaks {
            vec![lemonfiber_contract::spoken(
                curate::CAPABILITY,
                curate::MAJOR,
            )]
        } else {
            Vec::new()
        },
        media_types: vec!["tv".to_owned()],
        ..a_service("filler", &stranger, ApiKind::Servarr)
    };
    let asking = |origin: &Origin, first_party: bool| crate::wiring::Filler {
        first_party,
        ..an_asker("asker", origin, (intake::CAPABILITY, intake::MAJOR))
    };
    let other = Origin::Plugin {
        named: "other".to_owned(),
    };
    for (filler, asker, expected) in [
        (curating(true), asking(&moved, true), None),
        (
            curating(false),
            asking(&moved, true),
            Some(Unmade::Uncontracted),
        ),
        (curating(true), asking(&other, false), Some(Unmade::Asked)),
    ] {
        let ask = crate::wiring::Ask {
            by: asker.id.clone(),
            capability: "library.curate".to_owned(),
            native: None,
            fillers: vec![filler.clone()],
        };
        let fillers = crate::wiring::Fillers::holding(vec![asker.clone(), filler.clone()]);
        let came = made(
            &fillers,
            (&asker, &asker, intake::CAPABILITY),
            &ask,
            &filler,
        )
        .map(|(connection, ..)| connection);
        match expected {
            None => assert_eq!(came, Ok(Connection::Fulfilment { kind: Kind::Tv })),
            Some(why) => assert_eq!(came, Err(why)),
        }
    }
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
        native: None,
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
