use super::declared;
use crate::ports::http::Method;
use crate::ports::service::{
    Addressee, Held, Holds, Item, Medium, Presentation, Statuses, BASE, ITEM,
};
use crate::screening::door::{located, Door};

mod door;
mod guard;

const ID: &str = "0123456789abcdef0123456789abcdef";

const DOOR: &str = "https://house.local:8920";

/// A guard enforcing the declaration enforces what the door holds the household to.
#[test]
fn the_declaration_is_one_a_guard_can_enforce() {
    assert_eq!(declared().checked(), Ok(declared()));
}

#[test]
fn every_byte_route_is_matched_at_the_root_and_under_both_prefixes_without_regard_to_case() {
    let patterns: Vec<String> = declared()
        .bytes
        .into_iter()
        .map(|path| path.pattern)
        .collect();
    assert_eq!(
        patterns,
        [
            "(?i)^(?:/emby|/mediabrowser)?/Videos/(?P<item>[^/]+)/Trickplay/[^/]+/tiles\\.m3u8/?$",
            "(?i)^(?:/emby|/mediabrowser)?/(?:Videos|Audio|LiveTv/LiveStreamFiles|LiveTv/LiveRecordings)/(?P<item>[^/]+)(?:/|$)",
            "(?i)^(?:/emby|/mediabrowser)?/Items/(?P<item>[^/]+)/(?:Images|Download|File)(?:/|$)",
        ]
    );
    for path in declared().bytes {
        assert_eq!(
            path.item,
            "(?i)^[0-9a-f]{8}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{12}$"
        );
    }
}

#[test]
fn only_the_two_routes_under_videos_that_name_no_item_pass_unasked() {
    let exempt: Vec<Vec<String>> = declared()
        .bytes
        .into_iter()
        .map(|path| path.exempt)
        .collect();
    assert_eq!(
        exempt,
        [
            Vec::new(),
            vec![
                "(?i)^(?:/emby|/mediabrowser)?/Videos/(?:ActiveEncodings|MergeVersions)$"
                    .to_owned()
            ],
            Vec::new(),
        ]
    );
}

#[test]
fn every_byte_route_refuses_another_item_and_a_trickplay_playlist_asks_about_its_source() {
    let naming: Vec<Vec<(String, Held)>> = declared()
        .bytes
        .into_iter()
        .map(|path| {
            path.naming
                .into_iter()
                .map(|named| (named.parameter, named.held))
                .collect()
        })
        .collect();
    let refused = vec![
        ("itemId".to_owned(), Held::Refused),
        ("LiveStreamId".to_owned(), Held::Refused),
    ];
    let mut trickplay = refused.clone();
    trickplay.push((
        "MediaSourceId".to_owned(),
        Held::Asked {
            shape: "^[0-9a-f]{32}$".to_owned(),
        },
    ));
    assert_eq!(naming, [trickplay, refused.clone(), refused]);
}

#[test]
fn a_picture_found_by_a_name_and_the_splash_screen_are_refused_unasked() {
    assert_eq!(
        declared().refused,
        [
            "(?i)^(?:/emby|/mediabrowser)?/(?:Artists|Genres|MusicGenres|Persons|Studios)/[^/]+/Images(?:/|$)",
            "(?i)^(?:/emby|/mediabrowser)?/Branding/Splashscreen/?$",
        ]
    );
}

#[test]
fn the_guard_asks_jellyfin_for_the_item_as_the_members_own_token_sees_it() {
    let question = declared().question;
    assert_eq!(
        question.as_ref().map(|asked| asked.method),
        Some(Method::Get)
    );
    assert_eq!(
        question.as_ref().map(|asked| asked.path.as_str()),
        Some("/Items/{item}")
    );
    assert_eq!(
        question.as_ref().map(|asked| asked.to),
        Some(Addressee::Upstream)
    );
    let answers = question.map(|asked| asked.answers);
    assert_eq!(
        answers.as_ref().map(|answers| answers.may.clone()),
        Some(vec![Statuses { from: 200, to: 299 }])
    );
    assert_eq!(
        answers.as_ref().map(|answers| answers.may_not.clone()),
        Some(vec![Statuses::only(404)])
    );
    assert_eq!(
        answers.as_ref().map(|answers| answers.no_session.clone()),
        Some(vec![Statuses::only(401)])
    );
    assert_eq!(
        answers.map(|answers| answers.not_from_there),
        Some(vec![Statuses::only(403)])
    );
}

#[test]
fn a_bearer_token_of_jellyfins_shape_is_put_in_jellyfins_own_form() {
    let token = declared().token;
    assert_eq!(
        token.presented,
        Presentation::Header {
            name: "Authorization".to_owned(),
            value: r#"MediaBrowser Token="{token}""#.to_owned(),
        }
    );
    assert_eq!(token.shape, "^[0-9a-f]{32}$");
    assert_eq!(token.parameters, ["ApiKey", "api_key"]);
}

#[test]
fn jellyfin_listens_on_its_own_port_and_its_own_encrypted_one() {
    let ports = declared().ports;
    assert_eq!((ports.upstream, ports.encrypted), (8096, Some(8920)));
}

#[test]
fn each_location_built_from_its_template_is_where_the_door_locates_the_item() {
    let item = Item {
        id: ID.to_owned(),
        title: "Heat".to_owned(),
        year: None,
        medium: Medium::Film,
        holds: Holds {
            poster: true,
            backdrop: true,
            plays: true,
        },
    };
    let door = Door::At {
        base: DOOR.to_owned(),
        fingerprint: "ab12".to_owned(),
    };
    let at = located(item, &door).at;
    let built = |template: &str| Some(template.replace(BASE, DOOR).replace(ITEM, ID));
    let locations = declared().locations;
    assert_eq!(at.stream_from, built(&locations.stream));
    assert_eq!(at.poster, built(&locations.poster));
    assert_eq!(at.backdrop, built(&locations.backdrop));
}
