//! `request.intake`: what a request service is asked, the service the household asks for
//! what it wants through.
//!
//! The service itself (who it signs in through, which curators it hands requests to,
//! who it knows and what it tells them); what one member may ask for and what becomes
//! of one request; where the person who made a request already hears from it; and the
//! notices everybody reads before they ask; and the titles beyond the house a member may
//! find and ask for.

use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{
    Address, Addressing, Approving, Asked, Asking, Detail, Endpoint, FulfilmentTarget, Headroom,
    Holding, HouseholdRequest, IdentitySource, MediaServerLink, Noticing, Page, Quota,
    RegisteredTarget, Requesting, Requests, Searching, Telling, Wish,
};

crate::contract! {
    /// `request.intake`: a request service.
    pub mod intake = "request.intake" @ 1 {
        impl Requests {
            /// Whether it has been set up already.
            fn initialized() -> bool;
            /// Sign the household in through an identity source, and finish setting up.
            fn configure_identity(refer source: &IdentitySource as IdentitySource) -> ();
            /// Whether it answers to the key it is asked with, as its owner.
            fn answers() -> ();
            /// Every request the household has made.
            fn requests() -> Vec<HouseholdRequest>;
            /// Give each of these identity-source members an account.
            fn link_members(slice members: &[String] as String) -> ();
            /// The account it holds for one identity-source member, if any.
            fn member_for(str media_server_id: &str) -> Option<String>;
            /// What one identity-source member may ask for, if it holds them.
            fn requesting(str media_server_id: &str) -> Option<Requesting>;
            /// Make what one member asks for wait for approval.
            fn approval_first(str id: &str) -> ();
            /// Take one member's account away, with everything it asked for.
            fn remove_member(str id: &str) -> ();
            /// What it tells the household about.
            fn telling() -> Telling;
            /// Set what it tells the household about.
            fn tell(refer telling: &Telling as Telling) -> ();
            /// The curators it hands requests to.
            fn fulfilment_targets() -> Vec<RegisteredTarget>;
            /// Hand it one more curator.
            fn add_fulfilment_target(refer target: &FulfilmentTarget as FulfilmentTarget) -> ();
            /// Point a curator it holds somewhere else, with a new key.
            fn move_fulfilment_target(
                refer held: &RegisteredTarget as RegisteredTarget,
                refer at: &Endpoint as Endpoint,
                str key: &str,
            ) -> ();
            /// Whether it reaches the curator of one kind at an endpoint with a key.
            fn test_fulfilment_target(
                value kind: Kind,
                refer at: &Endpoint as Endpoint,
                str key: &str,
            ) -> ();
            /// Where it reaches the identity source after sign-in, and with what.
            fn media_server_link() -> MediaServerLink;
            /// Reach the identity source somewhere else, with a new key.
            fn link_media_server(refer at: &Endpoint as Endpoint, str key: &str) -> ();
        }
        impl Approving {
            /// What the household may ask for where nobody chose otherwise.
            fn asking() -> Asking;
            /// Set what the household may ask for.
            fn set_asking(refer asking: &Asking as Asking) -> ();
            /// What one member has left to ask for.
            fn left(str id: &str) -> Headroom;
            /// Hold one member to a quota, or to none.
            fn set_quota(str id: &str, value quota: Option<Quota>) -> ();
            /// Whether what one member asks for arrives without approval.
            fn approves_own(str id: &str, value may: bool) -> ();
            /// Approve or turn down one waiting request.
            fn decide(value request: i64, value approve: bool) -> ();
            /// Take one member's asking away, and say what was taken.
            fn hold_requests(str id: &str) -> Holding;
            /// Give one member back what was taken.
            fn release_requests(str id: &str, value holding: Holding) -> ();
        }
        impl Addressing {
            /// Where the person who made one request already hears from the service.
            fn reachable(value request: i64) -> Vec<Address>;
        }
        impl Noticing {
            /// Show exactly these notices, in this order.
            fn set_notices(slice notices: &[String] as String) -> ();
        }
        impl Searching {
            /// One page of the titles of these kinds a term finds.
            fn search(str term: &str, slice kinds: &[Kind] as Kind, value page: u32) -> Page;
            /// What one title is, with its certification in a region.
            fn detail(value kind: Kind, str id: &str, str region: &str) -> Option<Detail>;
            /// Ask for a title on behalf of one member.
            fn ask(str member: &str, refer wish: &Wish as Wish) -> Asked;
        }
    }
}
