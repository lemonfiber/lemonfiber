//! How the request service's own records are read.
//!
//! Its shapes rather than this product's: what it calls a page, what it puts in one,
//! and the two statuses it keeps apart, read from its numbers into the contract's words.
//! The two stay apart, because what became of a request and what became of the media it
//! asked for are separate facts.
//!
//! In a file of its own because `seerr.rs` is the client — signing in, pointing the
//! service at things, asking it questions — and this is the vocabulary one of those
//! answers comes back in.

use serde::Deserialize;

use crate::ports::service::{HouseholdRequest, MediaStatus, RequestStatus};
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
/// filing it knows it by, which is the exact join a name is found through, and, once the
/// media server holds it, when it arrived there and the identifier it is held under.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MediaRecord {
    #[serde(default)]
    pub(super) status: u8,
    #[serde(default)]
    pub(super) external_service_id: Option<i64>,
    #[serde(default)]
    pub(super) media_added_at: Option<String>,
    #[serde(default)]
    pub(super) jellyfin_media_id: Option<String>,
}

/// The member who asked: the name Seerr shows them by, and the media server's id for
/// them where they signed in through it.
///
/// The name is the account's own to change, so it says who asked and the id is what
/// tells whose request it is.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MemberRecord {
    #[serde(default)]
    pub(super) display_name: String,
    #[serde(default)]
    pub(super) jellyfin_user_id: Option<String>,
}

impl RequestRecord {
    /// The request as the port carries it, with the media type read into the service
    /// that files it — television or film, or nothing for a kind this build does not know.
    pub(super) fn into_request(self) -> HouseholdRequest {
        HouseholdRequest {
            id: self.id,
            made: self.created_at,
            member: self.requested_by.display_name,
            member_id: self.requested_by.jellyfin_user_id,
            kind: kind_of(&self.media_type),
            item: self.media.external_service_id,
            arrived: self.media.media_added_at,
            shelf_id: self.media.jellyfin_media_id,
            request_status: request_status(self.status),
            media_status: media_status(self.media.status),
        }
    }
}

/// The service's word for `kind`.
pub(super) const fn media_type(kind: Kind) -> &'static str {
    match kind {
        Kind::Tv => "tv",
        Kind::Movies => "movie",
    }
}

/// The kind the service's word names, or nothing for a person or anything else.
pub(super) fn kind_of(word: &str) -> Option<Kind> {
    Kind::ALL.into_iter().find(|kind| media_type(*kind) == word)
}

/// What became of a request, from the number the service files it under, or `None` for
/// a number it does not document.
pub(super) const fn request_status(number: u8) -> Option<RequestStatus> {
    match number {
        1 => Some(RequestStatus::Pending),
        2 => Some(RequestStatus::Approved),
        3 => Some(RequestStatus::Declined),
        4 => Some(RequestStatus::Failed),
        5 => Some(RequestStatus::Completed),
        _ => None,
    }
}

/// What became of the media a request asked for, from the service's number, or `None`
/// for one it does not document. Six is the service's "blacklisted", which no request
/// lemonfiber reads is ever in.
pub(super) const fn media_status(number: u8) -> Option<MediaStatus> {
    match number {
        1 => Some(MediaStatus::Unknown),
        2 => Some(MediaStatus::Pending),
        3 => Some(MediaStatus::Processing),
        4 => Some(MediaStatus::PartlyAvailable),
        5 => Some(MediaStatus::Available),
        7 => Some(MediaStatus::Deleted),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
