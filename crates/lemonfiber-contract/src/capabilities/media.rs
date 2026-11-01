//! `media.serve`: what a media server is asked, the service the household watches what it
//! holds on.
//!
//! What the house holds and what one member may watch; what is playing; one title, what
//! is part-way and how far it got; a title's pictures; opening and closing a device's own
//! session; whether
//! the library holds something yet; how each series it holds is filed, and reading one
//! item afresh; standing behind the front door, with the keys it holds for the stack's
//! own services; what a guard in front of it enforces; the certificate its internal
//! TLS port presents; and a library for each media type the stack files.

use lemonfiber_ports::media::{Kind, MEDIA};
use lemonfiber_ports::service::{
    AppKeys, Applied, Dated, Declared, Declaring, Encrypting, FiledAt, Fronted, Furnished,
    Furnishing, HowFar, Image, Item, ItemDetail, ItemProgress, Library, Picture, Playback,
    Screening, SeriesHeld, Upkeep, PICTURE_MOST, SERIES_MOST,
};
use serde_json::Value;

use super::{FOLDER, NOWHERE, PROXY};
use crate::conformance::Placeholder;

/// The most a picture's answer carries: its bytes written as base64, and room for the rest.
const PICTURE_ANSWER: usize = PICTURE_MOST.div_ceil(3) * 4 + 1024;

/// The most one series' line in an answer carries.
const SERIES_LINE: usize = 512;

/// The most an answer of every series carries.
const SERIES_ANSWER: usize = SERIES_MOST as usize * SERIES_LINE;

/// The device the conformance cases open a session for.
const DEVICE: &str = "conformance-device";

/// The app the conformance cases file a key under.
const APP: &str = "lemonfiber-conformance";

/// How many the conformance cases ask a list for at most.
const MOST: u32 = 10;

/// How far into an item the conformance cases say a member got, in seconds.
const POSITION: u64 = 60;

/// A term no library holds anything for.
const TERM: &str = "conformance";

/// An origin a browser reads it from.
const ORIGIN: &str = "https://watch.example";

/// The household's first account, as `identity.source` answered it.
fn member() -> String {
    Placeholder::answer(
        super::identity::source::CAPABILITY,
        super::identity::source::MAJOR,
        "household",
        "/0/id",
    )
    .written()
}

/// The key this contract's `mint` case answered.
fn minted() -> String {
    Placeholder::answer(serve::CAPABILITY, serve::MAJOR, "mint", "").written()
}

crate::contract! {
    /// `media.serve`: a media server.
    pub mod serve = "media.serve" @ 1 holds [
        crate::Held::kept("libraries", "Libraries as the operator arranged them"),
        crate::Held::kept("watch-history", "Watch history and progress"),
        crate::Held::kept("devices", "Devices signed in"),
    ] {
        impl Screening {
            /// Open a session on one member's account for one of their devices.
            fn signed_in(str member: &str, str device: &str) -> Option<String> where secret = &[""], asks = Asked {
                member: member(),
                device: DEVICE.to_owned(),
            };
            /// One title, as one member may see it, or as anybody may.
            fn title(opt_str member: Option<&str>, str id: &str) -> Option<ItemDetail> where asks = Asked {
                member: None,
                id: NOWHERE.to_owned(),
            };
            /// What one member has part-way through.
            fn part_way(str member: &str, value most: u32) -> Vec<ItemProgress> where asks = Asked {
                member: member(),
                most: MOST,
            };
            /// Record how far one member got through one item.
            fn progressed(str member: &str, str id: &str, refer how_far: &HowFar as HowFar) -> () where asks = Asked {
                member: member(),
                id: NOWHERE.to_owned(),
                how_far: HowFar { position: POSITION, ended: false },
            };
            /// Sign one device out of the session it was opened.
            fn sign_out(str device: &str) -> () where asks = Asked { device: DEVICE.to_owned() };
            /// One of a title's pictures, as one member may see it.
            fn picture(opt_str member: Option<&str>, str id: &str, value which: Picture) -> Option<Image> where largest = PICTURE_ANSWER, asks = Asked {
                member: None,
                id: NOWHERE.to_owned(),
                which: Picture::Poster,
            };
            /// What one member may watch, or what an account with every library holds.
            fn holdings(opt_str member: Option<&str>, value most: u32) -> Vec<Item> where asks = Asked {
                member: None,
                most: MOST,
            };
            /// What is playing, for one member or everybody.
            fn playing(opt_str member: Option<&str>) -> Vec<Playback> where asks = Asked { member: None };
        }
        impl Library {
            /// Whether the library holds an item of one kind for a term yet.
            fn has_item(value kind: Kind, str term: &str) -> bool where asks = Asked {
                kind: Kind::Movies,
                term: TERM.to_owned(),
            };
            /// Look over the library again.
            fn rescan() -> () where asks = Asked {};
        }
        impl Upkeep {
            /// Every series it holds, with how many seasons and episodes each is filed with.
            fn series_held(value most: u32) -> Vec<SeriesHeld> where largest = SERIES_ANSWER, asks = Asked { most: MOST };
            /// Read one item, and everything filed under it, afresh.
            fn refresh(str id: &str) -> () where asks = Asked { id: NOWHERE.to_owned() };
        }
        impl AppKeys {
            /// Every key filed under an app's name.
            fn filed_as(str app: &str) -> Vec<String> where asks = Asked { app: APP.to_owned() };
            /// Mint a key filed under an app's name.
            fn mint(str app: &str) -> String where secret = &[""], asks = Asked { app: APP.to_owned() };
            /// When each key filed under an app's name was made and last used.
            fn dated(str app: &str) -> Vec<Dated> where asks = Asked { app: APP.to_owned() };
            /// Whether it answers to a key alone.
            fn answers_to(str key: &str) -> () where asks = Asked { key: minted() };
            /// Revoke a key.
            fn revoke(str key: &str) -> () where asks = Asked { key: minted() };
        }
        impl Fronted {
            /// The addresses trusted to name the client.
            fn known_proxies() -> Vec<String> where asks = Asked {};
            /// Trust one address alone to name the client.
            fn trust_only(str address: &str) -> () where asks = Asked { address: PROXY.to_owned() };
            /// The origins a browser may read it from.
            fn allowed_origins() -> Vec<String> where asks = Asked {};
            /// Allow one origin alone.
            fn allow_only(str origin: &str) -> () where asks = Asked { origin: ORIGIN.to_owned() };
            /// Start again, so it reads what was written.
            fn restart() -> () where asks = Asked {};
        }
        impl Declaring {
            /// What a guard in front of it enforces, and where it serves each item.
            fn declared() -> Declared where asks = Asked {};
        }
        impl Furnishing {
            /// Add a library for each media type whose folder exists and no library reads, and say which a library reads.
            fn furnish(slice filed: &[FiledAt] as FiledAt) -> Vec<Furnished> where asks = Asked {
                filed: vec![FiledAt { media_type: MEDIA[0].to_owned(), folder: FOLDER.to_owned() }],
            };
        }
        impl Encrypting {
            /// Serve the internal TLS port with the PKCS#12 at this path inside the upstream's container, whose certificate has this SHA-256.
            fn serve_tls(str path: &str, str pin: &str) -> Applied where asks = Asked {
                path: Placeholder::Certificate.written(),
                pin: Placeholder::Pinned.written(),
            };
        }
    }
}

/// The operation each media type is given a library by.
const FURNISH: &str = "furnish";

/// `media.serve`, the operation giving a library asked once for each media type, with
/// the folder the conformance cases file into.
#[must_use]
pub fn capability() -> crate::Capability {
    let mut served = serve::capability();
    for operation in served
        .operations
        .iter_mut()
        .filter(|operation| operation.name == FURNISH)
    {
        operation.filed = MEDIA
            .iter()
            .map(|media_type| {
                let mut asked = operation.asks.clone();
                if let Some(at) = asked.pointer_mut("/filed/0/media_type") {
                    *at = Value::String((*media_type).to_owned());
                }
                (*media_type, asked)
            })
            .collect();
    }
    served
}
