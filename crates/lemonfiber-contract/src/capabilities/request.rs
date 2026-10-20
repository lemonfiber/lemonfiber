//! `request.intake`: what a request service is asked, the service the household asks for
//! what it wants through; and `request.gate`: what the gate it reaches the curators and
//! the media server through is asked.
//!
//! The request service itself (who it signs in through, which curators it hands requests
//! to, who it knows and what it tells them); what one member may ask for and what becomes
//! of one request; where the person who made a request already hears from it; the
//! notices everybody reads before they ask; and the titles beyond the house a member may
//! find and ask for. The gate is handed every route it admits at once, each with the
//! credential it presents and the tokens it admits, and answers each credential by its
//! fingerprint alone.

use std::collections::BTreeSet;

use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{
    Address, Addressing, Admitted, Approving, Asked, Asking, Credential, Detail, Endpoint,
    FulfilmentTarget, Gated, Gating, Headroom, Holding, HouseholdRequest, IdentitySource,
    MediaServerLink, Noticing, Occasion, Page, Protocol, QualityProfile, Quota, RegisteredTarget,
    Requesting, Requests, Searching, Target, Telling, Wish,
};

use super::{curator_address, CURATOR, CURATOR_PORT, FOLDER, NAMED, TOKEN_HASH};
use crate::conformance::Placeholder;

/// Where the conformance cases say the identity source is reached.
const IDENTITY_AT: &str = "http://conformance-identity";

/// The quality profile the conformance cases hand a curator.
const PROFILE: u32 = 1;

/// A member, a request and a title no request service holds.
const NOBODY: &str = "0";

/// A request no request service holds.
const NO_REQUEST: i64 = 0;

/// How many requests over how many days the conformance cases allow.
const QUOTA: Quota = Quota {
    requests: 5,
    days: 7,
};

/// The region the conformance cases ask a title's detail for.
const REGION: &str = "US";

/// The page of results the conformance cases ask for.
const PAGE: u32 = 1;

/// Where the conformance cases reach a curator or the identity source.
fn endpoint() -> Endpoint {
    Endpoint {
        host: CURATOR.to_owned(),
        port: CURATOR_PORT,
        base: String::new(),
    }
}

/// The string at `pointer` in this contract's answer to `operation`.
fn earlier(operation: &str, pointer: &str) -> String {
    Placeholder::answer(intake::CAPABILITY, intake::MAJOR, operation, pointer).written()
}

/// The account the request service holds for the member the conformance cases link.
fn member() -> String {
    earlier("member_for", "")
}

crate::contract! {
    /// `request.intake`: a request service.
    pub mod intake = "request.intake" @ 1 {
        impl Requests {
            /// Whether it has been set up already.
            fn initialized() -> bool where asks = Asked {};
            /// Sign the household in through an identity source, and finish setting up.
            fn configure_identity(refer source: &IdentitySource as IdentitySource) -> () where asks = Asked {
                source: IdentitySource {
                    at: IDENTITY_AT.to_owned(),
                    protocol: Protocol(NAMED.to_owned()),
                    credential: Credential::UserPass {
                        username: NAMED.to_owned(),
                        password: Placeholder::Minted.written(),
                    },
                },
            };
            /// Whether it answers to the key it is asked with, as its owner.
            fn answers() -> () where asks = Asked {};
            /// Every request the household has made.
            fn requests() -> Vec<HouseholdRequest> where asks = Asked {};
            /// Give each of these identity-source members an account.
            fn link_members(slice members: &[String] as String) -> () where asks = Asked { members: vec![NAMED.to_owned()] };
            /// The account it holds for one identity-source member, if any.
            fn member_for(str media_server_id: &str) -> Option<String> where asks = Asked { media_server_id: NAMED.to_owned() };
            /// What one identity-source member may ask for, if it holds them.
            fn requesting(str media_server_id: &str) -> Option<Requesting> where asks = Asked { media_server_id: NAMED.to_owned() };
            /// Make what one member asks for wait for approval.
            fn approval_first(str id: &str) -> () where asks = Asked { id: member() };
            /// Take one member's account away, with everything it asked for.
            fn remove_member(str id: &str) -> () where asks = Asked { id: NOBODY.to_owned() };
            /// What it tells the household about.
            fn telling() -> Telling where asks = Asked {};
            /// Set what it tells the household about.
            fn tell(refer telling: &Telling as Telling) -> () where asks = Asked {
                telling: Telling {
                    enabled: true,
                    occasions: BTreeSet::from([Occasion::Arrived]),
                    others: false,
                },
            };
            /// Hand it one more curator.
            fn add_fulfilment_target(refer target: &FulfilmentTarget as FulfilmentTarget) -> () where asks = Asked {
                target: FulfilmentTarget {
                    name: NAMED.to_owned(),
                    at: endpoint(),
                    moved_from: None,
                    key: Placeholder::Minted.written(),
                    kind: Kind::Tv,
                    profile: QualityProfile {
                        id: PROFILE,
                        name: NAMED.to_owned(),
                    },
                    folder: FOLDER.to_owned(),
                },
            };
            /// The curators it hands requests to.
            fn fulfilment_targets() -> Vec<RegisteredTarget> where asks = Asked {};
            /// Point a curator it holds somewhere else, with a new key.
            fn move_fulfilment_target(
                refer held: &RegisteredTarget as RegisteredTarget,
                refer at: &Endpoint as Endpoint,
                str key: &str,
            ) -> () where asks = Asked {
                held: RegisteredTarget {
                    id: earlier("fulfilment_targets", "/0/id"),
                    at: endpoint(),
                    key: earlier("fulfilment_targets", "/0/key"),
                    kind: Kind::Tv,
                },
                at: endpoint(),
                key: Placeholder::Minted.written(),
            };
            /// Whether it reaches the curator of one kind at an endpoint with a key.
            fn test_fulfilment_target(
                value kind: Kind,
                refer at: &Endpoint as Endpoint,
                str key: &str,
            ) -> () where asks = Asked { kind: Kind::Tv, at: endpoint(), key: Placeholder::Minted.written() };
            /// Where it reaches the identity source after sign-in, and with what.
            fn media_server_link() -> MediaServerLink where asks = Asked {};
            /// Reach the identity source somewhere else, with a new key.
            fn link_media_server(refer at: &Endpoint as Endpoint, str key: &str) -> () where asks = Asked { at: endpoint(), key: Placeholder::Minted.written() };
        }
        impl Approving {
            /// What the household may ask for where nobody chose otherwise.
            fn asking() -> Asking where asks = Asked {};
            /// Set what the household may ask for.
            fn set_asking(refer asking: &Asking as Asking) -> () where asks = Asked { asking: Asking { approves_own: false, quota: Some(QUOTA) } };
            /// What one member has left to ask for.
            fn left(str id: &str) -> Headroom where asks = Asked { id: member() };
            /// Hold one member to a quota, or to none.
            fn set_quota(str id: &str, value quota: Option<Quota>) -> () where asks = Asked { id: member(), quota: Some(QUOTA) };
            /// Whether what one member asks for arrives without approval.
            fn approves_own(str id: &str, value may: bool) -> () where asks = Asked { id: member(), may: false };
            /// Approve or turn down one waiting request.
            fn decide(value request: i64, value approve: bool) -> () where asks = Asked { request: NO_REQUEST, approve: false };
            /// Take one member's asking away, and say what was taken.
            fn hold_requests(str id: &str) -> Holding where asks = Asked { id: member() };
            /// Give one member back what was taken.
            fn release_requests(str id: &str, value holding: Holding) -> () where asks = Asked { id: member(), holding: Holding { taken: 0 } };
        }
        impl Addressing {
            /// Where the person who made one request already hears from the service.
            fn reachable(value request: i64) -> Vec<Address> where asks = Asked { request: NO_REQUEST };
        }
        impl Noticing {
            /// Show exactly these notices, in this order.
            fn set_notices(slice notices: &[String] as String) -> () where asks = Asked { notices: vec![NAMED.to_owned()] };
        }
        impl Searching {
            /// One page of the titles of these kinds a term finds.
            fn search(str term: &str, slice kinds: &[Kind] as Kind, value page: u32) -> Page where asks = Asked { term: NAMED.to_owned(), kinds: vec![Kind::Movies], page: PAGE };
            /// What one title is, with its certification in a region.
            fn detail(value kind: Kind, str id: &str, str region: &str) -> Option<Detail> where asks = Asked { kind: Kind::Movies, id: NOBODY.to_owned(), region: REGION.to_owned() };
            /// Ask for a title on behalf of one member.
            fn ask(str member: &str, refer wish: &Wish as Wish) -> Asked where asks = Asked {
                member: member(),
                wish: Wish {
                    id: NOBODY.to_owned(),
                    kind: Kind::Movies,
                    seasons: Vec::new(),
                },
            };
        }
    }
}

/// Where the conformance cases' gate reaches the media server.
const MEDIA_SERVER_AT: &str = "http://conformance-media-server";

/// The path segment the conformance cases' gate answers the media server's route under.
const MEDIA_SERVER_PATH: &str = "media-server";

/// The media server's major the conformance cases' gate forwards to.
const MEDIA_SERVER_MAJOR: u32 = 10;

/// One route the conformance cases' gate admits to `target` at `address`, under `path`,
/// forwarding to `majors`.
fn gated(target: Target, path: &str, address: String, majors: Vec<u32>) -> Gated {
    Gated {
        target,
        path: path.to_owned(),
        address,
        majors,
        credential: Placeholder::Minted.written(),
        tokens: vec![TOKEN_HASH.to_owned()],
    }
}

crate::contract! {
    /// `request.gate`: the gate the request service reaches through.
    pub mod gate = "request.gate" @ 1 {
        impl Gating {
            /// Admit every route in place of the ones it admits.
            fn admit(slice routes: &[Gated] as Gated) -> () where asks = Asked {
                routes: vec![
                    gated(
                        Target::Curator { media_type: Kind::Tv.media_type().to_owned() },
                        Kind::Tv.media_type(),
                        curator_address(),
                        Vec::new(),
                    ),
                    gated(
                        Target::MediaServer,
                        MEDIA_SERVER_PATH,
                        MEDIA_SERVER_AT.to_owned(),
                        vec![MEDIA_SERVER_MAJOR],
                    ),
                ],
            };
            /// Every route it admits, each credential by its fingerprint.
            fn admitted() -> Vec<Admitted> where asks = Asked {};
        }
    }
}
