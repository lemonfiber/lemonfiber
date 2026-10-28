use super::{named, reached, spelled, Naming, Ordinals};
use crate::config::{ADMIN_PASSWORD_SUFFIX, API_KEY_SUFFIX, PASSWORD_SUFFIX};

fn owned(words: &[&str]) -> Vec<String> {
    words.iter().map(|one| (*one).to_owned()).collect()
}

struct Service {
    id: &'static str,
    provides: Vec<String>,
    media_types: Vec<String>,
}

impl Service {
    fn new(id: &'static str, provides: &[&str], media_types: &[&str]) -> Self {
        Self {
            id,
            provides: owned(provides),
            media_types: owned(media_types),
        }
    }

    fn naming(&self) -> Naming<'_> {
        Naming {
            id: self.id,
            provides: &self.provides,
            media_types: &self.media_types,
        }
    }
}

fn stack() -> Vec<Service> {
    vec![
        Service::new("sonarr", &["library.curate"], &["tv"]),
        Service::new("radarr", &["library.curate"], &["movies"]),
        Service::new(
            "jellyfin",
            &["media.serve", "identity.source"],
            &["movies", "tv"],
        ),
        Service::new(
            "audiobookshelf",
            &["media.serve"],
            &["audiobooks", "podcasts"],
        ),
        Service::new("qbittorrent", &["download.torrent"], &[]),
        Service::new("prowlarr", &["indexer.search"], &[]),
        Service::new("nzbhydra2", &["indexer.search"], &[]),
    ]
}

fn name_of(ordinals: &Ordinals, id: &str, holds: &str) -> Option<String> {
    let services = stack();
    let namings: Vec<Naming<'_>> = services.iter().map(Service::naming).collect();
    let service = namings.iter().find(|one| one.id == id).copied()?;
    named(&namings, ordinals, service, holds)
}

#[test]
fn a_credential_is_named_for_the_capability_it_reaches_and_what_tells_its_fillers_apart() {
    let none = Ordinals::default();
    assert_eq!(
        name_of(&none, "sonarr", API_KEY_SUFFIX).as_deref(),
        Some("LIBRARY_CURATE_TV_API_KEY")
    );
    assert_eq!(
        name_of(&none, "radarr", API_KEY_SUFFIX).as_deref(),
        Some("LIBRARY_CURATE_MOVIES_API_KEY")
    );
    assert_eq!(
        name_of(&none, "jellyfin", ADMIN_PASSWORD_SUFFIX).as_deref(),
        Some("IDENTITY_SOURCE_ADMIN_PASSWORD")
    );
    assert_eq!(
        name_of(&none, "jellyfin", API_KEY_SUFFIX).as_deref(),
        Some("MEDIA_SERVE_API_KEY")
    );
    assert_eq!(
        name_of(&none, "audiobookshelf", PASSWORD_SUFFIX).as_deref(),
        Some("MEDIA_SERVE_AUDIOBOOKS_PASSWORD")
    );
    assert_eq!(
        name_of(&none, "qbittorrent", PASSWORD_SUFFIX).as_deref(),
        Some("DOWNLOAD_TORRENT_PASSWORD")
    );
}

#[test]
fn two_services_sharing_a_name_are_named_only_once_their_ordinals_are_recorded() {
    let none = Ordinals::default();
    assert_eq!(name_of(&none, "prowlarr", API_KEY_SUFFIX), None);
    assert_eq!(name_of(&none, "nzbhydra2", ADMIN_PASSWORD_SUFFIX), None);

    let services = stack();
    let namings: Vec<Naming<'_>> = services.iter().map(Service::naming).collect();
    assert_eq!(none.taken(&namings, "prowlarr", API_KEY_SUFFIX), 1);
    assert_eq!(none.taken(&namings, "nzbhydra2", API_KEY_SUFFIX), 2);

    let mut recorded = Ordinals::default();
    recorded.record("nzbhydra2", "indexer.search", 1);
    recorded.record("prowlarr", "indexer.search", 2);
    assert_eq!(
        name_of(&recorded, "nzbhydra2", ADMIN_PASSWORD_SUFFIX).as_deref(),
        Some("INDEXER_SEARCH_ADMIN_PASSWORD")
    );
    assert_eq!(
        name_of(&recorded, "prowlarr", API_KEY_SUFFIX).as_deref(),
        Some("INDEXER_SEARCH_2_API_KEY")
    );
    assert_eq!(recorded.taken(&namings, "prowlarr", API_KEY_SUFFIX), 2);
}

#[test]
fn a_name_two_credentials_would_both_take_is_given_to_neither_and_named() {
    let mut services = stack();
    services.push(Service::new("jackett", &["indexer.search"], &[]));
    let namings: Vec<Naming<'_>> = services.iter().map(Service::naming).collect();
    let mut both_first = Ordinals::default();
    both_first.record("prowlarr", "indexer.search", 1);
    both_first.record("nzbhydra2", "indexer.search", 1);
    both_first.record("jackett", "indexer.search", 1);
    let prowlarr = namings.iter().find(|one| one.id == "prowlarr").copied();
    let nzbhydra2 = namings.iter().find(|one| one.id == "nzbhydra2").copied();

    assert_eq!(
        prowlarr.and_then(|one| named(&namings, &both_first, one, API_KEY_SUFFIX)),
        None
    );
    assert_eq!(
        nzbhydra2.and_then(|one| named(&namings, &both_first, one, API_KEY_SUFFIX)),
        None
    );
    assert_eq!(
        super::clashing(&namings, &both_first),
        [
            "INDEXER_SEARCH_API_KEY",
            "INDEXER_SEARCH_PASSWORD",
            "INDEXER_SEARCH_USERNAME",
            "INDEXER_SEARCH_ADMIN_PASSWORD"
        ]
    );
}

#[test]
fn a_capability_the_vocabulary_does_not_carry_or_a_qualifier_that_is_no_media_type_names_nothing() {
    let invented = Service::new("invented", &["library.curate-tv"], &[]);
    assert_eq!(reached(invented.naming(), API_KEY_SUFFIX), None);
    let namespaced = Service::new("own", &["own:thing"], &[]);
    assert_eq!(reached(namespaced.naming(), API_KEY_SUFFIX), None);
    let numbered = Service::new("numbered", &["library.curate"], &["tv2"]);
    assert_eq!(reached(numbered.naming(), API_KEY_SUFFIX), None);
    let shaped = Service::new("shaped", &["media.serve"], &["audio-books"]);
    assert_eq!(reached(shaped.naming(), PASSWORD_SUFFIX), None);
}

#[test]
fn no_two_capabilities_the_vocabulary_carries_spell_alike() {
    let names: Vec<String> = lemonfiber_plugin::vocabulary::carried()
        .iter()
        .map(|one| spelled(one.name, None, None, ""))
        .collect();
    let mut unique = names.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), names.len(), "{names:?}");
    assert!(
        names
            .iter()
            .all(|name| !name.starts_with(crate::config::PLUGIN_SETTING)),
        "no capability is spelled in the namespace a plugin's credentials are kept in"
    );
    for media in lemonfiber_manifest::container::MEDIA {
        assert!(media.chars().all(|one| one.is_ascii_lowercase()), "{media}");
    }
}

#[test]
fn a_service_providing_nothing_has_no_credential_name() {
    let alone = Service::new("lonely", &[], &[]);
    assert_eq!(reached(alone.naming(), API_KEY_SUFFIX), None);
    assert_eq!(
        named(
            &[alone.naming()],
            &Ordinals::default(),
            alone.naming(),
            API_KEY_SUFFIX
        ),
        None
    );
    assert_eq!(Ordinals::default().taken(&[], "absent", API_KEY_SUFFIX), 1);
    assert_eq!(
        Ordinals::default().taken(&[alone.naming()], "lonely", API_KEY_SUFFIX),
        1
    );
}

#[test]
fn ordinals_are_written_as_they_are_read_and_a_pair_they_cannot_read_is_left_out() {
    let read = Ordinals::read(Some(
        "nzbhydra2@indexer.search=2, prowlarr@indexer.search=1,broken,lonely=1,x@y=0,z@w=many",
    ));
    assert_eq!(read.of("nzbhydra2", "indexer.search"), Some(2));
    assert_eq!(read.of("prowlarr", "indexer.search"), Some(1));
    assert_eq!(read.of("x", "y"), None);
    assert_eq!(
        read.written(),
        "nzbhydra2@indexer.search=2,prowlarr@indexer.search=1"
    );
    assert_eq!(Ordinals::read(None), Ordinals::default());
    assert_eq!(
        spelled("request.gate", Some("on-demand"), Some(3), "_KEY"),
        "REQUEST_GATE_ON_DEMAND_3_KEY"
    );
}
