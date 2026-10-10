//! Where the next page of a search begins.
//!
//! Written into the place itself rather than kept here, so the command line and the web
//! surface page alike and nothing is remembered between pages: the page of the request
//! service's to ask next, and every title already shown, so none is shown twice.

use base64::Engine as _;

use crate::error::codes::wish::NOT_A_PLACE;
use crate::error::Problem;

/// The most pages of the request service's a search goes through before it ends.
pub(super) const PAGES_AT_MOST: u32 = 25;

/// The most titles one search shows before it ends.
pub(super) const SHOWN_AT_MOST: usize = 500;

/// The longest a place is written: room for [`SHOWN_AT_MOST`] ids of the length a
/// request service gives them, and short enough to carry in an address.
const LONGEST: usize = 16 * 1024;

/// The request service's first page.
const FIRST: u32 = 1;

/// Where a search goes on from.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Place {
    /// The page of the request service's to ask next.
    pub(super) page: u32,
    /// Every title already shown, by id.
    pub(super) shown: Vec<String>,
}

impl Place {
    /// The place `after` names, or the start of a search where it names none.
    ///
    /// # Errors
    ///
    /// The refusal for a place no page of a search gave.
    pub(super) fn read(after: Option<&str>) -> Result<Self, Box<Problem>> {
        let Some(written) = after else {
            return Ok(Self {
                page: FIRST,
                shown: Vec::new(),
            });
        };
        (written.len() <= LONGEST)
            .then(|| {
                base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .decode(written)
                    .ok()
            })
            .flatten()
            .and_then(|bytes| serde_json::from_slice::<Self>(&bytes).ok())
            .filter(|place| {
                (FIRST + 1..=PAGES_AT_MOST).contains(&place.page)
                    && place.shown.len() <= SHOWN_AT_MOST
            })
            .ok_or_else(|| super::refused(NOT_A_PLACE, "That search can't go on from there."))
    }

    /// The place as a page names it, or nothing where the search ends here: past its
    /// last page, or with more shown than a place carries.
    pub(super) fn written(&self) -> Option<String> {
        if self.page > PAGES_AT_MOST || self.shown.len() > SHOWN_AT_MOST {
            return None;
        }
        serde_json::to_vec(self)
            .ok()
            .map(|bytes| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
            .filter(|written| written.len() <= LONGEST)
    }
}

#[cfg(test)]
mod tests;
