//! How the request service's own records are read.
//!
//! Its shapes rather than this product's: what it calls a page, what it puts in one,
//! and the two statuses it keeps apart. They are carried as the service's own numbers
//! and turned into the household's words above this, because what became of a request
//! and what became of the media it asked for are separate facts.
//!
//! In a file of its own because `seerr.rs` is the client — signing in, pointing the
//! service at things, asking it questions — and this is the vocabulary one of those
//! answers comes back in.

use serde::Deserialize;

use crate::ports::service::HouseholdRequest;
use crate::recyclarr::Kind;

/// How many requests are read per page. Seerr answers ten at a time unless told
/// otherwise, so a household of any size would take a walk; this asks for a generous
/// page and walks on only where the service's own total says there is more.
pub(super) const REQUEST_PAGE: usize = 100;

/// A page of the household's requests, and the totals that say whether there are more.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RequestPage {
    #[serde(default)]
    pub(super) page_info: PageInfo,
    #[serde(default)]
    pub(super) results: Vec<RequestRecord>,
}

/// How many requests there are in total, so the walk knows when it has them all.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PageInfo {
    #[serde(default)]
    pub(super) results: usize,
}

/// One request as Seerr records it: the number it is filed under, when it was asked
/// for, what became of it, what became of the media it asked for, who asked, and —
/// once it has been handed over — which \*arr item it is.
///
/// The number is read because a decision has to name one, and the date because two
/// questions turn on it: how long somebody has been waiting on an answer, and when the
/// window a count runs over lets go of this request.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RequestRecord {
    #[serde(default)]
    pub(super) id: i64,
    #[serde(default)]
    pub(super) created_at: Option<String>,
    #[serde(default)]
    pub(super) status: u8,
    #[serde(default, rename = "type")]
    pub(super) media_type: String,
    #[serde(default)]
    pub(super) media: MediaRecord,
    #[serde(default)]
    pub(super) requested_by: MemberRecord,
}

/// The media a request asked for. It carries no title — Seerr looks those up from a
/// metadata service rather than storing them — but it does carry the id the \*arr
/// filing it knows it by, which is the exact join a name is found through.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MediaRecord {
    #[serde(default)]
    pub(super) status: u8,
    #[serde(default)]
    pub(super) external_service_id: Option<i64>,
}

/// The member who asked, under the display name Seerr shows them by.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MemberRecord {
    #[serde(default)]
    pub(super) display_name: String,
}

impl RequestRecord {
    /// The request as the port carries it, with the media type read into the service
    /// that files it — television or film, or nothing for a kind this build does not know.
    pub(super) fn into_request(self) -> HouseholdRequest {
        HouseholdRequest {
            id: self.id,
            made: self.created_at,
            member: self.requested_by.display_name,
            kind: match self.media_type.as_str() {
                "tv" => Some(Kind::Sonarr),
                "movie" => Some(Kind::Radarr),
                _ => None,
            },
            item: self.media.external_service_id,
            request_status: self.status,
            media_status: self.media.status,
        }
    }
}
