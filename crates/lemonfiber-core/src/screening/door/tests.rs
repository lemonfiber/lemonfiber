use super::{
    at_the_door, directory, ensured, household_port, located, upstream, Door, NOT_MADE, NO_ADDRESS,
    UNREADABLE,
};
use crate::certificate::Unkept;
use crate::model::Located;
use crate::ports::service::{Holds, Item, Medium};
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
    }
}

const EVERYTHING: Holds = Holds {
    poster: true,
    backdrop: true,
    plays: true,
};

#[test]
fn a_film_with_both_pictures_is_located_whole_and_pinned() {
    let at = located(held(Medium::Film, EVERYTHING), &the_door()).at;
    assert_eq!(
        at.poster.as_deref(),
        Some("https://house.local:8920/Items/0123456789abcdef0123456789abcdef/Images/Primary")
    );
    assert_eq!(
        at.backdrop.as_deref(),
        Some("https://house.local:8920/Items/0123456789abcdef0123456789abcdef/Images/Backdrop")
    );
    assert!(at.stream_from.as_deref().is_some_and(|stream| stream
        .starts_with("https://house.local:8920/Videos/0123456789abcdef0123456789abcdef/master.m3u8?MediaSourceId=0123456789abcdef0123456789abcdef&")));
    assert_eq!(
        at.door.map(|door| door.fingerprint).as_deref(),
        Some("ab12")
    );
    assert_eq!(at.unlocated, None);
}

#[test]
fn a_series_streams_nothing_itself_and_a_missing_picture_is_not_located() {
    let at = located(
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
    let at = located(held(Medium::Other, EVERYTHING), &the_door()).at;
    assert_eq!(at.stream_from, None);
    let at = located(held(Medium::Episode, EVERYTHING), &the_door()).at;
    assert!(at.stream_from.is_some());
}

#[test]
fn a_door_nothing_is_known_about_locates_nothing_and_says_why() {
    let at = located(
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
    let kept = ensured(&dir).ok();
    let fingerprint = kept.as_ref().map(|kept| kept.fingerprint.clone());
    assert!(directory(&dir).join("certificate.pem").exists());
    assert_eq!(
        ensured(&dir).ok().map(|again| again.fingerprint),
        fingerprint,
        "a second start made the door a new certificate"
    );

    assert_eq!(
        at_the_door(Some("http://house.local:8920".to_owned()), Ok(kept.clone())),
        Door::At {
            base: "https://house.local:8920".to_owned(),
            fingerprint: fingerprint.unwrap_or_default(),
        }
    );
    assert_eq!(
        at_the_door(None, Ok(kept)),
        Door::Unknown(NO_ADDRESS.to_owned())
    );
    assert_eq!(
        at_the_door(Some("http://h:8920".to_owned()), Ok(None)),
        Door::Unknown(NOT_MADE.to_owned())
    );
    assert_eq!(
        at_the_door(
            Some("http://h:8920".to_owned()),
            Err(Unkept::Unreadable("torn".to_owned()))
        ),
        Door::Unknown(format!("{UNREADABLE} torn"))
    );
}

#[test]
fn the_doors_upstream_address_is_read_from_the_compose_file_that_fixes_it() {
    let dir = Scratch::named("door-upstream").kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(dir.join("compose"));
    assert_eq!(
        upstream(&dir),
        None,
        "a stack with no compose file has a door"
    );
    let _ = std::fs::write(
        dir.join("compose/media.yml"),
        "services:\n  door:\n    networks:\n      door:\n        ipv4_address: 10.80.96.2\n      door-upstream:\n        ipv4_address: 10.80.96.18\n",
    );
    assert_eq!(upstream(&dir).as_deref(), Some("10.80.96.18"));
    let _ = std::fs::write(dir.join("compose/media.yml"), "services:\n  jellyfin: {}\n");
    assert_eq!(upstream(&dir), None);
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
    static STACKLET: include_dir::Dir<'_> =
        include_dir::include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/stacklet");
    let ctx = crate::test_support::a_context()
        .over(crate::stack::Source::Embedded(&STACKLET))
        .build();
    assert_eq!(
        super::standing(&ctx).await,
        Door::Unknown(super::NO_STACK.to_owned())
    );
}

#[test]
fn an_item_named_by_anything_but_an_item_id_is_not_located() {
    for id in [
        "f1",
        "../System/Info",
        "0123456789abcdef0123456789abcdef&x=1",
    ] {
        let at = located(
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
