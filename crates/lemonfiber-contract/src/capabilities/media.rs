//! `media.serve`: what a media server is asked, the service the household watches what it
//! holds on.
//!
//! What the house holds and what one member may watch; what is playing; one title, what
//! is part-way and how far it got; opening and closing a device's own session; whether
//! the library holds something yet; and standing behind the front door, with the keys it
//! holds for the stack's own services.

use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{
    AppKeys, Dated, Fronted, HowFar, Item, ItemDetail, ItemProgress, Library, Playback, Screening,
};

crate::contract! {
    /// `media.serve`: a media server.
    pub mod serve = "media.serve" @ 1 {
        impl Screening {
            /// Open a session on one member's account for one of their devices.
            fn signed_in(str member: &str, str device: &str) -> Option<String>;
            /// One title, as one member may see it, or as anybody may.
            fn title(opt_str member: Option<&str>, str id: &str) -> Option<ItemDetail>;
            /// What one member has part-way through.
            fn part_way(str member: &str, value most: u32) -> Vec<ItemProgress>;
            /// Record how far one member got through one item.
            fn progressed(str member: &str, str id: &str, refer how_far: &HowFar as HowFar) -> ();
            /// Sign one device out of the session it was opened.
            fn sign_out(str device: &str) -> ();
            /// What one member may watch, or what an account with every library holds.
            fn holdings(opt_str member: Option<&str>, value most: u32) -> Vec<Item>;
            /// What is playing, for one member or everybody.
            fn playing(opt_str member: Option<&str>) -> Vec<Playback>;
        }
        impl Library {
            /// Whether the library holds an item of one kind for a term yet.
            fn has_item(value kind: Kind, str term: &str) -> bool;
            /// Look over the library again.
            fn rescan() -> ();
        }
        impl Fronted {
            /// The addresses trusted to name the client.
            fn known_proxies() -> Vec<String>;
            /// Trust one address alone to name the client.
            fn trust_only(str address: &str) -> ();
            /// The origins a browser may read it from.
            fn allowed_origins() -> Vec<String>;
            /// Allow one origin alone.
            fn allow_only(str origin: &str) -> ();
            /// Start again, so it reads what was written.
            fn restart() -> ();
        }
        impl AppKeys {
            /// Every key filed under an app's name.
            fn filed_as(str app: &str) -> Vec<String>;
            /// When each key filed under an app's name was made and last used.
            fn dated(str app: &str) -> Vec<Dated>;
            /// Mint a key filed under an app's name.
            fn mint(str app: &str) -> String;
            /// Whether it answers to a key alone.
            fn answers_to(str key: &str) -> ();
            /// Revoke a key.
            fn revoke(str key: &str) -> ();
        }
    }
}
