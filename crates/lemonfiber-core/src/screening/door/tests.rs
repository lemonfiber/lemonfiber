use super::{
    at_the_door, directory, drawn_play, ensured, household_port, located, passed, served_at, told,
    upstream, Door, GUARDED, NOT_AN_ITEM, NOT_MADE, NO_ADDRESS, REFUSED_DECLARATION, UNDECLARED,
    UNPLAYABLE, UNREADABLE,
};
use crate::app::Asker;
use crate::certificate::Unkept;
use crate::model::Located;
use crate::ports::service::{Holds, Item, Medium};
use lemonfiber_fixtures::declaring::{enforceable, Declares};
use lemonfiber_fixtures::ports::Chance;
use lemonfiber_fixtures::scratch::Scratch;

const ID: &str = "0123456789abcdef0123456789abcdef";

fn held(medium: Medium, holds: Holds) -> Item {
    Item {
        id: ID.to_owned(),
        title: "Heat".to_owned(),
        year: None,
        medium,
        holds,
    }
}

fn the_door() -> Door {
    Door::At {
        base: "https://house.local:8920".to_owned(),
        fingerprint: "ab12".to_owned(),
        locations: enforceable().locations,
        asker: Asker::Operator,
    }
}

/// `item` located at `door`, each play drawn from letters cycled.
fn located_at(item: Item, door: &Door) -> crate::model::Held {
    located(item, door, &Chance::cycling())
}

const EVERYTHING: Holds = Holds {
    poster: true,
    backdrop: true,
    plays: true,
};

#[test]
fn a_film_with_both_pictures_is_located_whole_and_pinned() {
    let at = located_at(held(Medium::Film, EVERYTHING), &the_door()).at;
    assert_eq!(
        at.poster.as_deref(),
        Some("https://house.local:8920/media/0123456789abcdef0123456789abcdef/poster")
    );
    assert_eq!(
        at.backdrop.as_deref(),
        Some("https://house.local:8920/media/0123456789abcdef0123456789abcdef/backdrop")
    );
    assert_eq!(
        at.stream_from.as_deref(),
        Some("https://house.local:8920/media/0123456789abcdef0123456789abcdef/bytes")
    );
    assert_eq!(
        at.door.map(|door| door.fingerprint).as_deref(),
        Some("ab12")
    );
    assert_eq!(at.unlocated, None);
}

#[test]
fn a_series_streams_nothing_itself_and_a_missing_picture_is_not_located() {
    let at = located_at(
        held(
            Medium::Series,
            Holds {
                poster: true,
                backdrop: false,
                plays: false,
            },
        ),
        &the_door(),
    )
    .at;
    assert!(at.poster.is_some());
    assert_eq!((at.backdrop, at.stream_from), (None, None));
}

#[test]
fn something_that_is_not_a_film_or_an_episode_does_not_stream() {
    let at = located_at(held(Medium::Other, EVERYTHING), &the_door()).at;
    assert_eq!(at.stream_from, None);
    let at = located_at(held(Medium::Episode, EVERYTHING), &the_door()).at;
    assert!(at.stream_from.is_some());
}

#[test]
fn a_door_nothing_is_known_about_locates_nothing_and_says_why() {
    let at = located_at(
        held(Medium::Film, EVERYTHING),
        &Door::Unknown("why".to_owned()),
    )
    .at;
    assert_eq!(
        at,
        Located {
            unlocated: Some("why".to_owned()),
            ..Located::default()
        }
    );
}

#[test]
fn the_door_is_https_on_its_own_port_only_with_an_address_and_a_certificate() {
    let dir = Scratch::named("door-made").kept();
    let _ = std::fs::remove_dir_all(&dir);
    let kept = ensured(&lemonfiber_adapters::Disk, &dir).ok();
    let fingerprint = kept.as_ref().map(|kept| kept.fingerprint.clone());
    assert!(directory(&dir).join("certificate.pem").exists());
    assert_eq!(
        ensured(&lemonfiber_adapters::Disk, &dir)
            .ok()
            .map(|again| again.fingerprint),
        fingerprint,
        "a second start made the door a new certificate"
    );

    assert_eq!(
        at_the_door(Some("http://house.local:8920".to_owned()), Ok(kept.clone())),
        Ok((
            "https://house.local:8920".to_owned(),
            fingerprint.unwrap_or_default()
        ))
    );
    assert_eq!(at_the_door(None, Ok(kept)), Err(NO_ADDRESS.to_owned()));
    assert_eq!(
        at_the_door(Some("http://h:8920".to_owned()), Ok(None)),
        Err(NOT_MADE.to_owned())
    );
    assert_eq!(
        at_the_door(
            Some("http://h:8920".to_owned()),
            Err(Unkept::Unreadable("torn".to_owned()))
        ),
        Err(format!("{UNREADABLE} torn"))
    );
}

#[test]
fn the_doors_upstream_address_is_the_one_the_projects_networks_fix_for_it() {
    let unguarded = crate::topology::Topology::default();
    assert_eq!(upstream(&unguarded), None);
    let guarded = crate::topology::derived(
        &[
            crate::topology::Member {
                id: "door",
                confined: false,
                publishes: true,
                inside: None,
                fronts: None,
            },
            crate::topology::Member {
                id: "jellyfin",
                confined: false,
                publishes: true,
                inside: None,
                fronts: None,
            },
        ],
        &[crate::topology::Settled {
            asker: "door",
            capability: "media.serve",
            reached: "jellyfin",
        }],
        &[],
        Some("door"),
        false,
    );
    assert_eq!(
        upstream(&guarded),
        Some(crate::topology::DOOR_ON_MEDIA.to_string())
    );
}

#[test]
fn the_household_reaches_the_media_server_at_the_doors_port_where_there_is_one() {
    let stack = crate::test_support::a_context().build();
    let manifest = stack.stack.checked_manifest(stack.today()).ok();
    let shipped = manifest
        .as_ref()
        .map(|manifest| household_port(manifest, 8095));
    let door = manifest.as_ref().and_then(|manifest| {
        manifest
            .services
            .iter()
            .find(|service| service.id == super::SERVICE)
            .and_then(|service| service.port)
    });
    assert_eq!(shipped, Some(door.unwrap_or(8095)));
}

/// With no stack directory there is no certificate to present, and the door says so
/// rather than locating anything.
#[tokio::test]
async fn with_no_stack_directory_the_door_is_unknown_and_says_why() {
    let ctx = crate::test_support::a_context()
        .over(crate::stack::Source::Embedded(
            &crate::test_support::STACKLET,
        ))
        .build();
    assert_eq!(
        super::standing(&ctx, &Declares::declaring(enforceable())).await,
        Door::Unknown(super::NO_STACK.to_owned())
    );
}

/// A door with an address and a certificate locates titles where the media server behind
/// it declared it serves them; without an address, or a declaration, it locates nothing
/// and says why.
#[tokio::test]
async fn a_standing_door_locates_where_the_server_behind_it_declared() {
    const HOUSEHOLD_HOST: &str = "192.168.1.20";
    let project = Scratch::new("door-standing");
    let fingerprint = ensured(&lemonfiber_adapters::Disk, project.path())
        .map(|kept| kept.fingerprint)
        .ok();
    let standing_at = |household_host: Option<&str>| {
        crate::test_support::a_context()
            .over(crate::stack::Source::Embedded(
                &crate::test_support::STACKLET,
            ))
            .settings(crate::config::Settings {
                stack_dir: Some(project.path().to_path_buf()),
                household_host: household_host.map(str::to_owned),
                ..crate::config::Settings::default()
            })
            .build()
    };
    let addressed = standing_at(Some(HOUSEHOLD_HOST));

    assert_eq!(
        super::standing(&addressed, &Declares::declaring(enforceable())).await,
        Door::At {
            base: format!("https://{HOUSEHOLD_HOST}:{GUARDED}"),
            fingerprint: fingerprint.unwrap_or_default(),
            locations: enforceable().locations,
        }
    );
    assert_eq!(
        super::standing(&addressed, &Declares::silent()).await,
        Door::Unknown(UNDECLARED.to_owned())
    );
    assert_eq!(
        super::standing(&standing_at(None), &Declares::declaring(enforceable())).await,
        Door::Unknown(NO_ADDRESS.to_owned())
    );
}

#[tokio::test]
async fn the_door_serves_where_the_media_server_declared_it_does() {
    assert_eq!(
        served_at(&Declares::declaring(enforceable())).await,
        Ok(enforceable().locations)
    );
}

#[tokio::test]
async fn a_media_server_that_will_not_say_where_it_serves_locates_nothing() {
    assert_eq!(
        served_at(&Declares::silent()).await,
        Err(UNDECLARED.to_owned())
    );
}

#[tokio::test]
async fn a_declaration_the_door_cannot_hold_a_server_to_locates_nothing() {
    let mut elsewhere = enforceable();
    elsewhere.locations.poster = "https://elsewhere.example/{item}".to_owned();
    let refused = served_at(&Declares::declaring(elsewhere)).await;
    assert!(
        refused
            .as_ref()
            .is_err_and(|why| why.starts_with(REFUSED_DECLARATION)),
        "{refused:?}"
    );
}

#[test]
fn an_item_named_by_anything_but_an_item_id_is_not_located() {
    for id in [
        "f1",
        "../System/Info",
        "0123456789abcdef0123456789abcdef&x=1",
    ] {
        let at = located_at(
            Item {
                id: id.to_owned(),
                ..held(Medium::Film, EVERYTHING)
            },
            &the_door(),
        )
        .at;
        assert_eq!(at.poster, None, "{id}");
        assert_eq!(at.stream_from, None, "{id}");
        assert_eq!(at.door, None, "{id}");
        assert!(at.unlocated.is_some(), "{id}");
    }
}

#[test]
fn a_member_is_told_only_that_it_does_not_play_here_yet_and_the_operator_what_to_fix() {
    assert_eq!(told(Asker::Operator, NO_ADDRESS.to_owned()), NO_ADDRESS);
    assert_eq!(told(Asker::Member, NO_ADDRESS.to_owned()), UNPLAYABLE);
    let member = Door::At {
        base: "https://house.local:8920".to_owned(),
        fingerprint: "ab12".to_owned(),
        locations: enforceable().locations,
        asker: Asker::Member,
    };
    let mut odd = held(Medium::Film, EVERYTHING);
    odd.id = "not an id".to_owned();
    assert_eq!(
        located_at(odd.clone(), &member).at.unlocated.as_deref(),
        Some(UNPLAYABLE)
    );
    assert_eq!(
        located_at(odd, &the_door()).at.unlocated.as_deref(),
        Some(NOT_AN_ITEM)
    );
}

#[test]
fn every_stream_location_names_a_play_of_its_own_drawn_fresh() {
    let mut locations = enforceable().locations;
    locations.stream = "{base}/media/{item}/bytes?play={play}".to_owned();
    let door = Door::At {
        base: "https://house.local:8920".to_owned(),
        fingerprint: "ab12".to_owned(),
        locations,
        asker: Asker::Operator,
    };
    let play = drawn_play(&Chance::cycling()).unwrap_or_default();
    assert_eq!(play, "6162636465666768696a6b6c6d6e6f70");
    assert_eq!(
        located_at(held(Medium::Film, EVERYTHING), &door)
            .at
            .stream_from
            .as_deref(),
        Some(
            format!(
                "https://house.local:8920/media/0123456789abcdef0123456789abcdef/bytes?play={play}"
            )
            .as_str()
        )
    );
    let nowhere = located(
        held(Medium::Film, EVERYTHING),
        &door,
        &Chance::exactly(None),
    );
    assert_eq!(nowhere.at.stream_from, None);
}

#[test]
fn a_pass_travels_in_its_one_parameter_beside_whatever_query_is_there() {
    assert_eq!(
        passed("/live/x/stream.ts", "p.1.q"),
        "/live/x/stream.ts?lfpass=p.1.q"
    );
    assert_eq!(
        passed("/videos/x/live.m3u8?LiveStreamId=ls1", "p.1.q"),
        "/videos/x/live.m3u8?LiveStreamId=ls1&lfpass=p.1.q"
    );
}
