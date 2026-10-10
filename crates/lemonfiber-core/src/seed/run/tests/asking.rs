use lemonfiber_manifest::{Api, ApiKind, KeySource};

use super::super::clients::Held;
use super::super::connecting::{pairings, Connection, Unmade};
use super::super::curating::{curators, wanted_clients};
use super::stack_root;
use crate::plugin::first_party::FirstParty;
use crate::plugin::{Asking, Installed, Placed};
use crate::ports::media::Kind;
use crate::ports::service::Credential;
use crate::test_support::{a_placed, an_installed};
use crate::wiring::{Chosen, Fillers};

const PLUGIN: &str = "helpers";

const MANIFEST: &str = "ab12";

const FETCHER: &str = "fetcher";

const SUBBER: &str = "subber";

/// A plugin service speaking `kind` on `listens`, asking for each of `asks`.
fn asker(service: &str, kind: ApiKind, listens: u16, asks: &[(&str, bool)]) -> Placed {
    let mut placed = a_placed(
        service,
        &[],
        Some(Api {
            kind,
            key_source: KeySource::ConfigXml,
            path: Some("/config/config.xml".to_owned()),
            version: Some(3),
        }),
        Some(listens),
    );
    placed.media_types = vec!["tv".to_owned()];
    placed.asks = asks
        .iter()
        .map(|(capability, each)| Asking {
            capability: (*capability).to_owned(),
            each: *each,
        })
        .collect();
    placed
}

/// The plugin: a curator asking for both download clients, and a subtitle finder asking
/// for every curator. Neither shares an id with anything the stack ships.
fn helpers() -> Installed {
    let mut installed = an_installed(
        PLUGIN,
        vec![
            asker(
                FETCHER,
                ApiKind::Servarr,
                8990,
                &[("download.usenet", false), ("download.torrent", false)],
            ),
            asker(SUBBER, ApiKind::Bazarr, 6768, &[("library.curate", true)]),
        ],
    );
    installed.manifest = MANIFEST.to_owned();
    installed
}

/// The shipped stack with the plugin installed, first-party where `trusted` says.
fn fillers(trusted: &[FirstParty]) -> Fillers {
    crate::test_support::stack()
        .manifest()
        .map(|manifest| {
            Fillers::trusting(
                &manifest,
                &[helpers()],
                &Chosen::default(),
                Some(stack_root()),
                trusted,
            )
        })
        .unwrap_or_default()
}

/// The plugin as this build's first-party set would hold it.
const FIRST_PARTY: [FirstParty; 1] = [FirstParty {
    plugin: PLUGIN,
    manifest: MANIFEST,
}];

/// What each pairing `by` asks for comes to, by the filler and the capability.
fn made(fillers: &Fillers, by: &str) -> Vec<(String, String, Result<Connection, Unmade>)> {
    pairings(fillers)
        .into_iter()
        .filter(|pairing| pairing.asker.id == by)
        .map(|pairing| {
            (
                pairing.filler.id.clone(),
                pairing.ask.capability.clone(),
                pairing.made.map(|(connection, _, _)| connection),
            )
        })
        .collect()
}

/// Both download clients' credentials, as the stack holds them.
fn held() -> Held {
    Held::from(std::collections::BTreeMap::from([
        ("sabnzbd".to_owned(), Credential::ApiKey("k".to_owned())),
        (
            "qbittorrent".to_owned(),
            Credential::UserPass {
                username: "admin".to_owned(),
                password: "p".to_owned(),
            },
        ),
    ]))
}

/// The download clients the plugin's curator is told about, where the seed takes it as
/// a curator at all.
fn told(fillers: &Fillers) -> Option<Vec<String>> {
    curators(fillers)
        .into_iter()
        .find(|curator| curator.id() == FETCHER)
        .map(|curator| {
            wanted_clients(curator, fillers, &held())
                .into_iter()
                .map(|client| client.name)
                .collect()
        })
}

#[test]
fn a_third_party_askers_every_pairing_is_withheld_and_it_is_told_of_no_client() {
    let fillers = fillers(&[]);
    for by in [FETCHER, SUBBER] {
        let made = made(&fillers, by);
        assert!(!made.is_empty(), "{by} is paired with what fills its asks");
        assert!(
            made.iter().all(|(_, _, made)| *made == Err(Unmade::Asked)),
            "{by}: {made:?}"
        );
    }
    assert_eq!(told(&fillers), Some(Vec::new()));
}

#[test]
fn a_first_party_curator_asking_for_downloads_is_told_of_each_client() {
    let fillers = fillers(&FIRST_PARTY);
    let made = made(&fillers, FETCHER);
    assert!(
        made.iter()
            .any(|(filler, capability, made)| filler == "sabnzbd"
                && capability == "download.usenet"
                && matches!(made, Ok(Connection::DownloadClient(_)))),
        "{made:?}"
    );
    assert!(
        made.iter()
            .any(|(filler, capability, made)| filler == "qbittorrent"
                && capability == "download.torrent"
                && matches!(made, Ok(Connection::DownloadClient(_)))),
        "{made:?}"
    );
    assert_eq!(
        told(&fillers).map(|told| told.len()),
        Some(2),
        "{:?}",
        told(&fillers)
    );
}

#[test]
fn a_first_party_subtitle_finder_is_paired_with_every_curator_by_its_own_id() {
    let fillers = fillers(&FIRST_PARTY);
    let made = made(&fillers, SUBBER);
    assert!(fillers
        .services()
        .filter(|one| one.id == SUBBER)
        .all(|one| one.brought_by() == Some(PLUGIN)));
    assert!(
        made.iter()
            .any(|(filler, capability, made)| filler == "sonarr"
                && capability == "library.curate"
                && *made == Ok(Connection::Subtitles(Kind::Tv))),
        "{made:?}"
    );
    assert!(
        made.iter().any(|(filler, _, made)| filler == "radarr"
            && *made == Ok(Connection::Subtitles(Kind::Movies))),
        "{made:?}"
    );
}
