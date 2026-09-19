//! What one member may watch, read from the media server as that account.
//!
//! Its own file because it is a different question from the one beside it. That one is
//! who holds an account and what each may do; this is what is *on the shelf* for one of
//! them — and the answer is the server's rather than this product's, which is the whole
//! of why it is worth a module of its own to say so once.

use super::{item_type, Jellyfin, Kind, Medium, Method};
use crate::ports::service::{Failure, Held};

/// One item as the media server describes it.
#[derive(serde::Deserialize)]
struct ItemResource {
    #[serde(rename = "Id", default)]
    id: String,
    #[serde(rename = "Name", default)]
    name: String,
    /// Absent wherever the server holds no year, which it does for anything it could
    /// not match against a catalogue.
    #[serde(rename = "ProductionYear", default)]
    year: Option<u16>,
    #[serde(rename = "Type", default)]
    medium: String,
}

/// A page of items.
#[derive(serde::Deserialize)]
struct ItemsResource {
    #[serde(rename = "Items", default)]
    items: Vec<ItemResource>,
}

/// What this member may watch, as the server answers it for them.
///
/// Beside the impl rather than inside it because it decides, and nothing inside an
/// `#[async_trait]` body is attributed to a line in the coverage report.
///
/// **Addressed to the member's account.** `/Users/{id}/Items` is the server applying
/// that account's own library access, age limit and blocked kinds before it answers.
/// The administrator's key authenticates the call, but the id in the path is what the
/// limits are read from — so nothing here re-applies them, and there is no second copy
/// of three rules to disagree with the server on the day any of them moves.
pub(super) async fn holdings(
    jellyfin: &Jellyfin,
    member: &str,
    most: u32,
) -> Result<Vec<Held>, Failure> {
    let kinds = Kind::ALL.map(item_type).join(",");
    let asked = format!(
        "/Users/{member}/Items?Recursive=true&IncludeItemTypes={kinds}\
         &SortBy=DateCreated&SortOrder=Descending&Limit={most}"
    );
    let request = jellyfin.as_admin(Method::Get, &asked, None).await?;
    let response = jellyfin.endpoint.send(&request).await?;
    let held: ItemsResource = jellyfin
        .endpoint
        .decode(&response, "what the household holds could not be read")?;

    Ok(held.items.into_iter().map(ItemResource::held).collect())
}

/// Which of the two a held item is, from the word the server uses for it.
///
/// Read through [`item_type`] rather than against words of its own, so the filter the
/// query asks for and the answer it reads back cannot drift apart.
fn medium(word: &str) -> Medium {
    if word == item_type(Kind::Radarr) {
        Medium::Film
    } else if word == item_type(Kind::Sonarr) {
        Medium::Series
    } else {
        Medium::Other
    }
}

impl ItemResource {
    /// The same item in this product's own words.
    fn held(self) -> Held {
        Held {
            id: self.id,
            title: self.name,
            year: self.year,
            medium: medium(&self.medium),
        }
    }
}
