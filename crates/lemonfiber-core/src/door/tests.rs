use super::fixtures::{asking, brought, fronted, installed, service, serving, watching};
use super::{begins_at, facing, Facing};
use crate::plugin::Installed;
use lemonfiber_manifest::Bind;

/// Where the household begins over `services` with `installed` beside them and nothing
/// filling an ask, as the facing and the id it runs under.
fn begins(
    services: &[lemonfiber_manifest::Service],
    installed: &[Installed],
) -> Option<(Facing, String)> {
    let candidates = super::candidates(services, installed, &crate::wiring::Fillers::default());
    begins_at(&candidates).map(|(facing, candidate)| (facing, candidate.id.to_owned()))
}

#[test]
fn the_request_surface_is_the_door_wherever_there_is_one() {
    assert_eq!(
        begins(&[watching()], &[asking()]),
        Some((Facing::Asking, "seerr".to_owned()))
    );
}

#[test]
fn the_library_is_the_door_only_where_nothing_can_be_asked_for() {
    assert_eq!(
        begins(&[watching()], &[]),
        Some((Facing::Watching, "door".to_owned()))
    );
}

#[test]
fn a_stack_publishing_nothing_to_the_household_has_no_door_at_all() {
    // An operator-only configuration. Answered as none rather than filled in
    // with the nearest thing that would open.
    let services = [
        service("sonarr", Some(Bind::Loopback)),
        service("homepage", Some(Bind::Lan)),
    ];
    assert_eq!(begins(&services, &[]), None);
}

#[test]
fn the_operators_index_is_never_offered_as_a_way_in() {
    let mut index = service("index", Some(Bind::Lan));
    index.provides = vec!["dashboard.show".to_owned()];
    assert_eq!(facing(&index), Some(Facing::Operators));
    assert_eq!(
        facing(&service("homepage", Some(Bind::Lan))),
        Some(Facing::Unstated),
        "no service is the index by its name"
    );
    assert!(!Facing::Operators.begins());
    assert!(Facing::Operators.because().contains("never a way in"));
}

#[test]
fn a_service_reachable_only_from_this_machine_is_no_part_of_this() {
    let admin = service("sonarr", Some(Bind::Loopback));
    assert_eq!(facing(&admin), None);
    let unpublished = service("recyclarr", None);
    assert_eq!(facing(&unpublished), None);
}

#[test]
fn the_proxy_carries_the_others_rather_than_being_one_of_them() {
    let mut proxy = service("front", Some(Bind::Lan));
    proxy.provides = vec!["proxy.front".to_owned()];
    assert_eq!(facing(&proxy), Some(Facing::Carriage));
    assert!(!Facing::Carriage.begins());
}

#[test]
fn a_published_service_nobody_vouched_for_is_offered_to_nobody() {
    let stranger = service("something-new", Some(Bind::Lan));
    assert_eq!(facing(&stranger), Some(Facing::Unstated));
    assert!(!Facing::Unstated.begins());
}

/// A stack service named for a request service or a library is what the register says of
/// it: only a contract makes a place to begin.
#[test]
fn a_service_is_no_door_by_its_name() {
    for id in ["seerr", "jellyfin", "caddy"] {
        let named = service(id, Some(Bind::Lan));
        assert_eq!(facing(&named), Some(Facing::Unstated), "{id}");
    }
}

#[test]
fn every_facing_says_why_it_is_or_is_not_a_way_in() {
    let said: Vec<&str> = [
        Facing::Asking,
        Facing::Watching,
        Facing::Shelf,
        Facing::Operators,
        Facing::Carriage,
        Facing::Unstated,
    ]
    .into_iter()
    .map(Facing::because)
    .collect();
    for because in &said {
        assert!(!because.is_empty());
    }
    let mut unique = said.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), said.len(), "each says something of its own");
}

/// What the plugin's first service is to the household, judged among `installed` alone.
fn judged(plugin: &Installed) -> Option<Facing> {
    let placed = plugin.services.first()?;
    super::brought(std::slice::from_ref(plugin), plugin, placed)
}

/// A plugin's service published to the household is judged by the contract the adapter in
/// front of it speaks: a request service asks, a media server watches, the proxy carries
/// the others, anything else is unstated, and the register does not reach a stranger's service by its id. One reachable
/// from this machine alone is no part of this.
#[test]
fn a_plugins_service_is_judged_by_the_contract_in_front_of_it() {
    for (plugin, expected) in [
        (
            fronted("requests", Some("request.intake@1"), Some("ask")),
            Some(Facing::Asking),
        ),
        (
            fronted("server", Some("media.serve@1"), Some("watch")),
            Some(Facing::Watching),
        ),
        (
            fronted("server", Some("media.serve@2"), Some("watch")),
            Some(Facing::Unstated),
        ),
        (
            fronted("front", Some("proxy.front@1"), Some("front")),
            Some(Facing::Carriage),
        ),
        (
            fronted("curator", Some("library.curate@1"), Some("curate")),
            Some(Facing::Unstated),
        ),
        (
            fronted("index", Some("dashboard.show@1"), Some("home")),
            Some(Facing::Operators),
        ),
        (
            fronted("homepage", None, Some("home")),
            Some(Facing::Unstated),
        ),
        (
            installed(vec![brought("requests", Some("ask"))]),
            Some(Facing::Unstated),
        ),
        (fronted("requests", Some("request.intake@1"), None), None),
    ] {
        assert_eq!(judged(&plugin), expected, "{:?}", plugin.services);
    }
}

/// An adapter speaking a contract in front of another plugin's service says nothing about
/// what that service is.
#[test]
fn only_the_services_own_plugin_says_what_it_is() {
    let requests = installed(vec![brought("requests", Some("ask"))]);
    let mut adapter = crate::test_support::a_placed("requests-adapter", &[], Some(8080));
    adapter.fronts = Some("requests".to_owned());
    adapter.speaks = vec!["request.intake@1".to_owned()];
    let both = [
        requests.clone(),
        crate::test_support::an_installed("elsewhere", vec![adapter]),
    ];
    let placed = requests.services.first();
    assert_eq!(
        placed.and_then(|placed| super::brought(&both, &requests, placed)),
        Some(Facing::Unstated)
    );
}

/// The stack's own request surface is the door before a plugin's, a plugin's request
/// surface is the door before the stack's library, and a plugin's library is the door
/// where nothing anywhere can be asked for.
#[test]
fn a_plugins_service_can_be_the_door_by_the_same_rule() {
    let requests = fronted("requests", Some("request.intake@1"), Some("ask"));
    let server = fronted("server", Some("media.serve@1"), Some("watch"));

    assert_eq!(
        begins(&[watching()], &[requests]),
        Some((Facing::Asking, "requests".to_owned()))
    );
    assert_eq!(
        begins(&[], &[server]),
        Some((Facing::Watching, "server".to_owned()))
    );
}

/// A media server declaring shelf media alone is a shelf, one declaring anything else
/// beside it is the library, and so is one declaring none.
#[test]
fn a_media_server_is_a_shelf_where_it_serves_shelf_media_alone() {
    for (media, expected) in [
        (&["audiobooks"][..], Facing::Shelf),
        (&["audiobooks", "podcasts"][..], Facing::Shelf),
        (&["music"][..], Facing::Shelf),
        (&["books", "comics"][..], Facing::Shelf),
        (&["movies", "audiobooks"][..], Facing::Watching),
        (&["tv"][..], Facing::Watching),
        (&["films"][..], Facing::Watching),
        (&[][..], Facing::Watching),
    ] {
        assert_eq!(
            judged(&serving("server", media)),
            Some(expected),
            "{media:?}"
        );
    }
}

/// A shelf is never somewhere to begin: where one stands beside the library and the
/// request service, the household begins at the request service, and where it stands
/// alone there is no door at all.
#[test]
fn a_shelf_stands_beside_the_door_rather_than_being_it() {
    let shelf = serving("listening", &["audiobooks", "podcasts"]);
    let library = serving("library", &["movies", "tv", "music"]);
    let requests = fronted("requests", Some("request.intake@1"), Some("ask"));

    assert_eq!(
        begins(&[], &[shelf.clone(), library.clone(), requests]),
        Some((Facing::Asking, "requests".to_owned()))
    );
    assert_eq!(
        begins(&[], &[shelf.clone(), library]),
        Some((Facing::Watching, "library".to_owned()))
    );
    assert_eq!(begins(&[], &[shelf]), None);
}

/// The shelf media are the vocabulary's, and every other medium in it is one a library
/// is watched for.
#[test]
fn the_shelf_media_are_the_vocabulary_but_what_is_watched() {
    let vocabulary = lemonfiber_manifest::container::MEDIA;
    assert!(super::SHELF_MEDIA
        .iter()
        .all(|one| vocabulary.contains(one)));
    let watched: Vec<&str> = vocabulary
        .into_iter()
        .filter(|one| !super::SHELF_MEDIA.contains(one))
        .collect();
    assert_eq!(watched, ["tv", "movies"]);
}

/// A plugin's service is reached through the proxy at its label, and one reachable from
/// this machine alone is reached at no port the household could use.
#[test]
fn a_plugins_service_is_reached_through_the_proxy() {
    let plugin = installed(vec![
        brought("requests", Some("ask")),
        brought("hidden", None),
    ]);
    let candidates = super::candidates(
        &[],
        std::slice::from_ref(&plugin),
        &crate::wiring::Fillers::default(),
    );
    let reached: Vec<super::Reached<'_>> = candidates.iter().map(|one| one.reached).collect();

    assert_eq!(
        reached,
        vec![super::Reached::Proxied("ask"), super::Reached::Port(None)]
    );
    assert_eq!(
        candidates.first().map(|one| one.name),
        Some("requests the plugin's")
    );
}

/// The stack this build carries with every part it installs: the request service moved
/// into its plugin is the door, judged by the contract its adapter speaks and reached on
/// the port it is published on as the stack's own was, the guard in front of the library
/// is met under the library's name, and the listening and music servers are shelves beside
/// them.
#[test]
fn the_carried_stack_begins_at_its_moved_request_service() {
    let manifest = lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK).ok();
    let services = manifest
        .as_ref()
        .map(|one| one.services.clone())
        .unwrap_or_default();
    let installed = crate::test_support::adapted::installed_parts();
    let fillers = manifest.as_ref().map(|manifest| {
        crate::wiring::Fillers::of(
            manifest,
            &installed,
            &crate::wiring::Chosen::default(),
            None,
        )
    });
    let candidates = super::candidates(&services, &installed, &fillers.unwrap_or_default());

    let mut ids: Vec<&str> = candidates.iter().map(|one| one.id).collect();
    let met = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), met, "a moved service is met once");
    assert_eq!(
        begins_at(&candidates).map(|(facing, door)| (facing, door.id, door.reached)),
        Some((Facing::Asking, "seerr", super::Reached::Port(Some(5055))))
    );
    let guard = candidates
        .iter()
        .find(|one| one.id == crate::screening::door::SERVICE)
        .map(|one| (one.name, one.facing));
    assert_eq!(guard, Some(("Jellyfin", Some(Facing::Watching))));
    let begin: Vec<&str> = candidates
        .iter()
        .filter(|one| one.facing.is_some_and(Facing::begins))
        .map(|one| one.id)
        .collect();
    assert_eq!(begin, ["door", "seerr"]);
    for shelf in ["audiobookshelf", "navidrome"] {
        let facing = candidates
            .iter()
            .find(|one| one.id == shelf)
            .and_then(|one| one.facing);
        assert_eq!(facing, Some(Facing::Shelf), "{shelf} is a shelf");
    }
}

/// A first-party part moved in place of one of the stack's services, published to the
/// household with no contract in front of it that says what it is, is unstated whatever
/// service it stands in for.
#[test]
fn a_moved_part_no_contract_settles_is_unstated() {
    let mut parts = crate::test_support::adapted::installed_parts();
    let moved = "homepage";
    for placed in parts
        .iter_mut()
        .filter(|one| one.plugin == "audiobookshelf")
        .flat_map(|one| one.services.iter_mut())
    {
        placed.speaks.clear();
        if placed.service == "audiobookshelf" {
            placed.service = moved.to_owned();
            placed.replaces = Some(moved.to_owned());
        }
    }
    let facing = parts
        .iter()
        .filter(|one| one.plugin == "audiobookshelf")
        .flat_map(|plugin| {
            plugin
                .services
                .iter()
                .filter(|placed| placed.service == moved)
                .map(|placed| super::brought(&parts, plugin, placed))
        })
        .next();
    assert_eq!(facing, Some(Some(Facing::Unstated)));
}

/// The stack's guard whose ask for `media.serve` two media servers contest, with nothing
/// settling it, is met under its own name: there is no one library behind it to name it
/// after.
#[test]
fn a_guard_no_one_library_fills_is_met_under_its_own_name() {
    let second = [crate::test_support::a_contracted_media_server(None)];
    let manifest = lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK)
        .ok()
        .map(|mut manifest| {
            for wiring in manifest
                .wirings
                .iter_mut()
                .filter(|one| one.asks.as_deref() == Some(super::serve::CAPABILITY))
            {
                wiring.filled_by = None;
            }
            manifest
        });
    let services = manifest
        .as_ref()
        .map(|one| one.services.clone())
        .unwrap_or_default();
    let fillers = manifest.as_ref().map(|manifest| {
        crate::wiring::Fillers::of(manifest, &second, &crate::wiring::Chosen::default(), None)
    });
    let candidates = super::candidates(&services, &second, &fillers.unwrap_or_default());
    let guard = candidates
        .iter()
        .find(|one| one.id == "door")
        .map(|one| (one.name, one.facing));

    assert_eq!(guard, Some(("Door", Some(Facing::Watching))));
}

/// A plugin declaring that it guards the media stream is neither the way to the library
/// nor named after it; the first-party door moved in place of the stack's is both.
#[test]
fn only_the_door_moved_in_place_is_the_way_to_the_library_and_named_after_it() {
    let mut stranger = crate::test_support::a_placed("guard", &["media.guard"], Some(8443));
    stranger.reached = brought("guard", Some("guard")).reached;
    let plugin = crate::test_support::an_installed("strangers-guard", vec![stranger]);
    let manifest = lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK).ok();
    let services = manifest.map(|one| one.services).unwrap_or_default();
    let candidates = super::candidates(
        &services,
        std::slice::from_ref(&plugin),
        &crate::wiring::Fillers::default(),
    );
    let met = candidates
        .iter()
        .find(|one| one.id == "guard")
        .map(|one| (one.name, one.facing));

    assert_eq!(met, Some(("guard the stand-in", Some(Facing::Unstated))));
}
