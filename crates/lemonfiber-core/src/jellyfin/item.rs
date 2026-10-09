//! One item as the media server describes it, as every read of items takes it.
//!
//! One shape for the shelf, a title's details and what a member was part-way through,
//! so the three cannot come to disagree about what an item is or what it holds.

use super::item_type;
use crate::ports::media::Kind;
use crate::ports::service::{Held, Holds, Located, Medium};

/// One item as the media server describes it.
#[derive(serde::Deserialize)]
pub(super) struct ItemResource {
    #[serde(rename = "Id", default)]
    pub(super) id: String,
    #[serde(rename = "Name", default)]
    name: String,
    /// Absent wherever the server holds no year, which it does for anything it could
    /// not match against a catalogue.
    #[serde(rename = "ProductionYear", default)]
    year: Option<u16>,
    #[serde(rename = "Type", default)]
    medium: String,
    #[serde(rename = "ImageTags", default)]
    images: Images,
    #[serde(rename = "BackdropImageTags", default)]
    backdrops: Vec<String>,
    /// Whether it holds other items rather than playing itself: a series, a season.
    #[serde(rename = "IsFolder", default)]
    folder: bool,
}

/// The pictures the server holds for an item, by kind.
#[derive(serde::Deserialize, Default)]
struct Images {
    #[serde(rename = "Primary", default)]
    primary: Option<String>,
}

/// A page of items.
#[derive(serde::Deserialize)]
pub(super) struct ItemsResource {
    #[serde(rename = "Items", default)]
    pub(super) items: Vec<ItemResource>,
}

impl ItemResource {
    /// The same item in this product's own words, located nowhere yet.
    pub(super) fn held(self) -> Held {
        Held {
            holds: Holds {
                poster: self.images.primary.is_some(),
                backdrop: !self.backdrops.is_empty(),
                plays: !self.folder,
            },
            id: self.id,
            title: self.name,
            year: self.year,
            medium: medium(&self.medium),
            at: Located::default(),
        }
    }

    /// Whether the server files it as a series.
    pub(super) fn is_a_series(&self) -> bool {
        self.medium == item_type(Kind::Sonarr)
    }
}

/// The word the server files an episode under.
const EPISODE: &str = "Episode";

/// Which of the kinds a held item is, from the word the server uses for it.
///
/// Read through [`item_type`] rather than against words of its own, so the filter a
/// query asks for and the answer it reads back cannot drift apart.
fn medium(word: &str) -> Medium {
    if word == item_type(Kind::Radarr) {
        Medium::Film
    } else if word == item_type(Kind::Sonarr) {
        Medium::Series
    } else if word == EPISODE {
        Medium::Episode
    } else {
        Medium::Other
    }
}

#[cfg(test)]
mod tests;
