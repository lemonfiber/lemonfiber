//! Where the request service will carry a sentence of this program's own.
//!
//! It has no notice board. What it has is a home page built of rows, each row a heading
//! over a strip of things to ask for, and an operator may add rows of their own and
//! title them freely. A row whose search matches nothing still draws its heading — so a
//! heading is a line of text this program can put in front of everybody who lands there,
//! which is everybody who is about to ask for something. Read off `/app/src/components/
//! Discover/index.tsx` and `/app/src/components/MediaSlider/index.tsx` in
//! `ghcr.io/seerr-team/seerr:v3.3.0`, and driven against it.
//!
//! **Only an administrator may write a row and anybody signed in may read them.** That
//! asymmetry is the whole opening: the household needs no account here to be told
//! something, because the account they already have on the request service is the one
//! that reads it.
//!
//! **Which rows are this program's is written in a field nobody reads.** A row carries a
//! heading and a search, and the search is what marks one as ours — matching on the
//! heading instead would orphan every notice the moment its wording was improved, and
//! leave the old sentence standing beside the new one.
//!
//! **Adding a row and showing it are two calls.** The service files a new row switched
//! off and last in the order, and the only call that switches one on rewrites the order
//! and the on-off state of *every* row it is sent. So the whole list is read, this
//! program's rows are put at the front of it, and it is written back whole — anything an
//! operator arranged for themselves comes back exactly as it was.

use async_trait::async_trait;
use serde::Deserialize;

use super::Seerr;
use crate::ports::http::Method;
use crate::ports::service::{Failure, Noticing};

/// Where the rows of the home page are read and written.
const SLIDERS: &str = "/settings/discover";

/// Where one new row is filed, switched off and last.
const ADD: &str = "/settings/discover/add";

/// What marks a row as this program's, written where a row keeps its search.
///
/// A search nothing matches, which is what leaves the row empty under its heading. It is
/// also the word a member lands on if they click the heading, which is the one place
/// this marker is visible at all — and a search for it finding nothing is the truthful
/// answer to what a notice has to offer.
const OURS: &str = "lemonfiber";

/// The kind of row whose search is free text rather than a genre or a studio.
///
/// `TMDB_SEARCH` in the service's own numbering. The kind matters only in that its
/// stored field is the one that can hold an arbitrary marker.
const SEARCHING: u8 = 17;

/// One row of the home page, in the service's own words.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Row {
    #[serde(default)]
    id: i64,
    #[serde(default, rename = "type")]
    kind: u8,
    #[serde(default)]
    is_built_in: bool,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    data: Option<String>,
}

impl Row {
    /// Whether this row is one this program put there.
    ///
    /// A row the service ships is never ours whatever it holds, so the built-in flag is
    /// asked first: the marker is a field an operator could type by hand, and the flag
    /// is one they cannot.
    fn ours(&self) -> bool {
        !self.is_built_in && self.data.as_deref() == Some(OURS)
    }

    /// The sentence this row is showing, where it is showing one.
    fn sentence(&self) -> String {
        self.title.clone().unwrap_or_default()
    }

    /// This row as the service wants it written back, switched on or left as found.
    ///
    /// Every field the write assigns is named, because that call rewrites the order and
    /// the on-off state of every row it is sent and a row left out of the body is a row
    /// switched off.
    fn written(&self, order: usize, enabled: bool) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "type": self.kind,
            "isBuiltIn": self.is_built_in,
            "enabled": enabled,
            "order": order,
            "title": self.title,
            "data": self.data,
        })
    }
}

impl Seerr {
    /// Every row of the home page, in the order the service holds them.
    async fn rows(&self) -> Result<Vec<Row>, Failure> {
        let response = self
            .endpoint
            .send(&self.request(Method::Get, SLIDERS, None))
            .await?;
        self.endpoint.decode(
            &response,
            "what the request service is showing the household could not be read",
        )
    }

    /// Put this program's rows at the front, switched on, and leave the rest as found.
    ///
    /// The list is read again rather than assembled from what was just written: the
    /// service assigns the numbers, and a body naming a row the service does not hold is
    /// a body that makes a second one.
    async fn shown(&self) -> Result<(), Failure> {
        let held = self.rows().await?;
        let (ours, theirs): (Vec<&Row>, Vec<&Row>) = held.iter().partition(|row| row.ours());
        // Position in the body is the order the page draws them in, so this program's
        // rows are simply put first. Everything else keeps the order it came in, and
        // keeps whether it was switched on: this write assigns both for every row it
        // names, and a row it does not name is a row switched off.
        let body: Vec<serde_json::Value> = ours
            .iter()
            .map(|row| (*row, true))
            .chain(theirs.iter().map(|row| (*row, row.enabled)))
            .enumerate()
            .map(|(order, (row, enabled))| row.written(order, enabled))
            .collect();
        // Plain data, so this cannot fail; an empty body on the impossible branch keeps
        // the write free of a line no test could reach.
        let written = self
            .endpoint
            .send(&self.request(
                Method::Post,
                SLIDERS,
                Some(serde_json::to_string(&body).unwrap_or_default()),
            ))
            .await?;
        self.endpoint.expect_success(&written)
    }
}

#[async_trait]
impl Noticing for Seerr {
    async fn set_notices(&self, notices: &[String]) -> Result<(), Failure> {
        let held = self.rows().await?;
        let showing: Vec<String> = held
            .iter()
            .filter(|row| row.ours())
            .map(Row::sentence)
            .collect();
        // Nothing to do is the ordinary case: this is asked every time the household is
        // read and the house has usually not changed its mind since. Writing anyway
        // would rearrange somebody's home page on every glance at it.
        if showing == notices {
            return Ok(());
        }
        for row in held.iter().filter(|row| row.ours()) {
            let path = format!("{SLIDERS}/{}", row.id);
            let removed = self
                .endpoint
                .send(&self.request(Method::Delete, &path, None))
                .await?;
            self.endpoint.expect_success(&removed)?;
        }
        for notice in notices {
            let body = serde_json::json!({
                "type": SEARCHING,
                "title": notice,
                "data": OURS,
            })
            .to_string();
            let added = self
                .endpoint
                .send(&self.request(Method::Post, ADD, Some(body)))
                .await?;
            self.endpoint.expect_success(&added)?;
        }
        self.shown().await
    }
}

#[cfg(test)]
mod tests {
    use super::{Row, OURS};

    /// A row the service ships is never this program's, whatever its search says.
    #[test]
    fn a_built_in_row_is_never_ours() {
        let row = Row {
            id: 1,
            kind: 1,
            is_built_in: true,
            enabled: true,
            title: None,
            data: Some(OURS.to_owned()),
        };

        assert!(
            !row.ours(),
            "a row the service ships was claimed by this program because of a field an \
             operator could have typed"
        );
    }

    /// A row somebody else added is left alone.
    #[test]
    fn a_row_somebody_else_added_is_not_ours() {
        let row = Row {
            id: 2,
            kind: 17,
            is_built_in: false,
            enabled: true,
            title: Some("Wessel's own row".to_owned()),
            data: Some("213".to_owned()),
        };

        assert!(
            !row.ours(),
            "somebody else's row was claimed by this program"
        );
    }

    /// A row with no heading reads as an empty sentence rather than as no row.
    #[test]
    fn a_row_with_no_heading_is_an_empty_sentence() {
        let row = Row {
            id: 3,
            kind: 17,
            is_built_in: false,
            enabled: true,
            title: None,
            data: Some(OURS.to_owned()),
        };

        assert!(row.ours());
        assert_eq!(row.sentence(), String::new());
    }

    /// What is written back names every field the write assigns.
    #[test]
    fn what_is_written_back_names_every_field_the_write_assigns() {
        let row = Row {
            id: 7,
            kind: 17,
            is_built_in: false,
            enabled: false,
            title: Some("a notice".to_owned()),
            data: Some(OURS.to_owned()),
        };

        let written = row.written(3, true);

        // The whole document rather than field by field: what the service is sent
        // is the shape as well as the values, and a field that should not be there
        // is as wrong as one that is missing.
        assert_eq!(
            written,
            serde_json::json!({
                "id": 7,
                "type": 17,
                "isBuiltIn": false,
                "enabled": true,
                "order": 3,
                "title": "a notice",
                "data": OURS,
            })
        );
    }
}
