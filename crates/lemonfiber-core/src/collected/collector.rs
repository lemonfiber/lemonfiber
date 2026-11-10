//! The curator a member's music or books are asked of, whichever medium it answers for.

use std::sync::Arc;

use lemonfiber_contract::capabilities::{books, music};

use super::{Medium, Unit};
use crate::ports::service::{AskedWith, BookAsk, Candidate, Candidates, Failure, MusicAsk};

/// A curator a member's music or books are asked of, over its medium's contract.
#[derive(Clone)]
pub(crate) enum Collector {
    /// A curator of music, over `music.curate`.
    Music(Arc<dyn music::curate::Fills>),
    /// A curator of books, over `books.curate`.
    Books(Arc<dyn books::curate::Fills>),
}

impl Collector {
    /// One page of the candidates of `unit` that `term` finds, from where `after` says.
    ///
    /// # Errors
    ///
    /// The [`Failure`] the curator answered with, and one for a unit of the other medium.
    pub(crate) async fn lookup(
        &self,
        unit: Unit,
        term: &str,
        after: Option<&str>,
    ) -> Result<Candidates, Failure> {
        match (self, unit) {
            (Self::Music(curator), Unit::Music(unit)) => curator.lookup(unit, term, after).await,
            (Self::Books(curator), Unit::Books(unit)) => curator.lookup(unit, term, after).await,
            _ => Err(other_medium()),
        }
    }

    /// The candidate of `unit` `reference` names, or nothing where the curator's catalogue
    /// holds no such reference.
    ///
    /// # Errors
    ///
    /// The [`Failure`] the curator answered with, and one for a unit of the other medium.
    pub(crate) async fn candidate(
        &self,
        unit: Unit,
        reference: &str,
    ) -> Result<Option<Candidate>, Failure> {
        match (self, unit) {
            (Self::Music(curator), Unit::Music(unit)) => curator.candidate(unit, reference).await,
            (Self::Books(curator), Unit::Books(unit)) => curator.candidate(unit, reference).await,
            _ => Err(other_medium()),
        }
    }

    /// The units of its medium the curator answers for as it is configured now.
    ///
    /// # Errors
    ///
    /// The [`Failure`] the curator answered with.
    pub(crate) async fn units(&self) -> Result<Vec<Unit>, Failure> {
        let (with, medium) = match self {
            Self::Music(curator) => (curator.profiles().await?, Medium::Music),
            Self::Books(curator) => (curator.profiles().await?, Medium::Books),
        };
        Ok(Unit::ALL
            .into_iter()
            .filter(|unit| unit.medium() == medium && with.units.contains(&unit.curated()))
            .collect())
    }

    /// Ask for the candidate `reference` names, as a `unit`, at the first root folder and
    /// profiles the curator holds, searching for it at once.
    ///
    /// # Errors
    ///
    /// The [`Failure`] the curator answered with, one for a curator holding no root folder
    /// or profile to ask with, and one for a unit of the other medium.
    pub(crate) async fn request(&self, unit: Unit, reference: &str) -> Result<Candidate, Failure> {
        match (self, unit) {
            (Self::Music(curator), Unit::Music(unit)) => {
                let with = curator.profiles().await?;
                let (root_folder, quality_profile) = first(&with)?;
                let metadata_profile = with.metadata.first().ok_or_else(nothing_to_ask_with)?.id;
                curator
                    .request(&MusicAsk {
                        unit,
                        reference: reference.to_owned(),
                        root_folder,
                        quality_profile,
                        metadata_profile,
                        search: true,
                    })
                    .await
            }
            (Self::Books(curator), Unit::Books(unit)) => {
                let with = curator.profiles().await?;
                let (root_folder, quality_profile) = first(&with)?;
                curator
                    .request(&BookAsk {
                        unit,
                        reference: reference.to_owned(),
                        root_folder,
                        quality_profile,
                        search: true,
                    })
                    .await
            }
            _ => Err(other_medium()),
        }
    }
}

/// What a failure here names the curator as.
const CURATOR: &str = "the curator";

/// The first root folder and quality profile `with` holds.
fn first(with: &AskedWith) -> Result<(String, i64), Failure> {
    let folder = with.root_folders.first().ok_or_else(nothing_to_ask_with)?;
    let quality = with.quality.first().ok_or_else(nothing_to_ask_with)?;
    Ok((folder.clone(), quality.id))
}

/// The failure for a curator holding no root folder or profile a request could name.
fn nothing_to_ask_with() -> Failure {
    Failure::Refused {
        service: CURATOR.to_owned(),
        detail: "it holds no root folder or profile a request could name".to_owned(),
    }
}

/// The failure for a unit of the medium this curator does not answer for.
fn other_medium() -> Failure {
    Failure::Unsupported {
        service: CURATOR.to_owned(),
        detail: "it answers for the other medium".to_owned(),
    }
}

#[cfg(test)]
mod tests;
