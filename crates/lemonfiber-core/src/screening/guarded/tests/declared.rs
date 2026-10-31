//! What a guard in front of Jellyfin enforces, as plugin-jellyfin's adapter declares it
//! over `media.serve`: the declaration a guard is held to in these tests.
//!
//! Jellyfin answers its byte routes without asking who is there, so the guard asks it
//! about the item first, as the member's own token sees it. It refuses outright what no
//! question about an item covers: a picture found by a name and the splash screen. On a
//! byte route it refuses a query's `itemId` and `LiveStreamId`, under which Jellyfin
//! serves another item than the path's, and on a trickplay playlist or tile it asks about
//! the item `MediaSourceId` names as well. Jellyfin serves every route at its root and under
//! `/emby` and `/mediabrowser` alike.

use crate::ports::http::Method;
use crate::ports::service::{
    Addressee, Answers, BytePath, Declared, Held, LivePath, Locations, Naming, Picture, Ports,
    Presentation, Question, Statuses, Token, BASE, ITEM, TOKEN,
};
use crate::screening::door::GUARDED;

/// The header Jellyfin reads a token from.
const AUTHORIZATION_HEADER: &str = crate::endpoint::AUTHORIZATION;

/// The status Jellyfin answers an item it holds none of with.
const ABSENT: u16 = 404;

/// The query a stream is asked for with: the item's own source, and the codecs every
/// supported player decodes, in segments both platforms play.
const STREAMED: &str = "VideoCodec=h264,hevc&AudioCodec=aac,mp3,ac3,eac3&SegmentContainer=mp4";

/// A token in the scheme [`AUTHORIZATION_HEADER`] is parsed by.
fn carrying(token: &str) -> String {
    format!(r#"MediaBrowser Token="{token}""#)
}

/// Where Jellyfin streams an item from, beneath its address.
fn streamed_at(id: &str) -> String {
    format!("/Videos/{id}/master.m3u8?MediaSourceId={id}&{STREAMED}")
}

/// Where Jellyfin serves one of a title's pictures, beneath its address.
fn pictured_at(id: &str, which: Picture) -> String {
    let named = match which {
        Picture::Poster => "Primary",
        Picture::Backdrop => "Backdrop",
    };
    format!("/Items/{id}/Images/{named}")
}

/// The port Jellyfin listens on.
const PORT: u16 = 8096;

/// What every route is matched under: without regard to case, at the root or beneath
/// either prefix Jellyfin strips.
const PREFIXED: &str = "(?i)^(?:/emby|/mediabrowser)?";

/// Where an item's id sits in a byte path: one whole segment.
const ITEM_SEGMENT: &str = "(?P<item>[^/]+)";

/// An item's id: 32 hexadecimal digits in either case, with or without the dashes
/// Jellyfin also reads.
const ITEM_ID: &str = "(?i)^[0-9a-f]{8}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{12}$";

/// 32 lower-case hexadecimal digits: the shape Jellyfin issues a token in, and writes a
/// playlist's source in.
const HEX: &str = "^[0-9a-f]{32}$";

/// The query parameters Jellyfin reads a token from.
const TOKEN_PARAMETERS: [&str; 2] = ["ApiKey", "api_key"];

/// The parameter under which Jellyfin's subtitle routes serve another item.
const ITEM_PARAMETER: &str = "itemId";

/// The parameter under which Jellyfin serves the bytes of a live stream opened on another
/// item.
const LIVE_STREAM_PARAMETER: &str = "LiveStreamId";

/// The parameter a trickplay playlist names its source by, which Jellyfin writes into
/// the playlists it hands out.
const SOURCE_PARAMETER: &str = "MediaSourceId";

/// The statuses Jellyfin answers an item a member may have with.
const MAY: Statuses = Statuses { from: 200, to: 299 };

/// The status Jellyfin answers a token that is absent, ended or wrong with.
const NO_SESSION: u16 = 401;

/// The status Jellyfin answers a member it will not serve from where they are with.
const NOT_FROM_THERE: u16 = 403;

/// The routes beneath which Jellyfin serves an item's streams, playlists, segments,
/// subtitles, attachments, trickplay and lyrics, and the two live-TV file routes.
const KEYED: &str = "(?:Videos|Audio|LiveTv/LiveStreamFiles|LiveTv/LiveRecordings)";

/// The two routes under `/Videos` that name no item, and on which Jellyfin requires a
/// session itself.
const UNKEYED: &str = "Videos/(?:ActiveEncodings|MergeVersions)$";

/// The holders of a picture Jellyfin finds by a name rather than an id.
const BY_NAME: &str = "(?:Artists|Genres|MusicGenres|Persons|Studios)";

/// The parameters every byte route refuses, each naming another item than the path's.
fn refusing() -> Vec<Naming> {
    [ITEM_PARAMETER, LIVE_STREAM_PARAMETER]
        .map(|parameter| Naming {
            parameter: parameter.to_owned(),
            held: Held::Refused,
        })
        .to_vec()
}

/// A byte path of `route`, its item checked against Jellyfin's id.
fn byte_path(route: &str, exempt: Vec<String>, naming: Vec<Naming>) -> BytePath {
    BytePath {
        pattern: format!("{PREFIXED}/{route}"),
        query: None,
        space: None,
        item: ITEM_ID.to_owned(),
        exempt,
        naming,
    }
}

/// What a guard in front of Jellyfin enforces, and where it serves each item.
pub(crate) fn declared() -> Declared {
    let mut trickplay = refusing();
    trickplay.push(Naming {
        parameter: SOURCE_PARAMETER.to_owned(),
        held: Held::Asked {
            shape: HEX.to_owned(),
        },
    });
    Declared {
        bytes: vec![
            byte_path(
                &format!("Videos/{ITEM_SEGMENT}/Trickplay(?:/|$)"),
                Vec::new(),
                trickplay,
            ),
            byte_path(
                &format!("{KEYED}/{ITEM_SEGMENT}(?:/|$)"),
                vec![format!("{PREFIXED}/{UNKEYED}")],
                refusing(),
            ),
            byte_path(
                &format!("Items/{ITEM_SEGMENT}/(?:Images|Download|File)(?:/|$)"),
                Vec::new(),
                refusing(),
            ),
        ],
        live: vec![LivePath {
            pattern: "^/LiveTv/LiveStreamFiles/(?P<stream>[^/]+)/stream\\.".to_owned(),
            stream: "^[0-9A-Za-z]{1,64}$".to_owned(),
        }],
        refused: vec![
            format!("{PREFIXED}/{BY_NAME}/[^/]+/Images(?:/|$)"),
            format!("{PREFIXED}/Branding/Splashscreen/?$"),
        ],
        question: Some(Question {
            method: Method::Get,
            path: format!("/Items/{ITEM}"),
            to: Addressee::Upstream,
            answers: Answers {
                may: vec![MAY],
                may_not: vec![Statuses::only(ABSENT)],
                no_session: vec![Statuses::only(NO_SESSION)],
                not_from_there: vec![Statuses::only(NOT_FROM_THERE)],
            },
        }),
        token: Token {
            presented: Presentation::Header {
                name: AUTHORIZATION_HEADER.to_owned(),
                value: carrying(TOKEN),
            },
            shape: HEX.to_owned(),
            parameters: TOKEN_PARAMETERS.map(str::to_owned).to_vec(),
            headers: Vec::new(),
        },
        locations: Locations {
            stream: format!("{BASE}{}", streamed_at(ITEM)),
            poster: format!("{BASE}{}", pictured_at(ITEM, Picture::Poster)),
            backdrop: format!("{BASE}{}", pictured_at(ITEM, Picture::Backdrop)),
        },
        ports: Ports {
            upstream: PORT,
            encrypted: Some(GUARDED),
            internal_tls: None,
        },
    }
}

mod tests;
