//! Titles beyond the house, as the request service finds them, and asking for one on a
//! member's behalf.
//!
//! Read off `ghcr.io/seerr-team/seerr:v3.5.0`: a search answers mixed pages of films,
//! series and people, each film and series carrying where it stands in the house; a
//! film's certifications are its release dates per region, a series' its content ratings;
//! a series counts its specials as season 0, which is never asked for. The owner's key may
//! file a request for any member by naming them.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::records::{kind_of, media_status, media_type, request_status};
use super::{Seerr, NOT_FOUND};
use crate::endpoint::query_encoded;
use crate::ports::http::Method;
use crate::ports::media::Kind;
use crate::ports::service::{
    Asked, Detail, Failure, Found, MediaStatus, Page, RequestStatus, Searching, Season, Wish,
};

/// Where every published poster is served from, at the width a phone shows it.
const POSTERS: &str = "https://image.tmdb.org/t/p/w342";

/// The status a film is filed under once it has come out.
const RELEASED: &str = "Released";

/// What a series is filed under before it has come out.
const UNRELEASED_SERIES: [&str; 3] = ["Planned", "In Production", "Pilot"];

/// The number a series files its specials under.
const SPECIALS: u32 = 0;

/// One page of a search, as the service answers it.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchPage {
    #[serde(default)]
    page: u32,
    #[serde(default)]
    total_pages: u32,
    #[serde(default)]
    results: Vec<Hit>,
}

/// One thing a search found: a film, a series or a person.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Hit {
    id: i64,
    media_type: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    release_date: Option<String>,
    #[serde(default)]
    first_air_date: Option<String>,
    #[serde(default)]
    poster_path: Option<String>,
    #[serde(default)]
    media_info: Option<MediaInfo>,
}

/// Where one title, or one season of it, stands in the house.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MediaInfo {
    #[serde(default)]
    status: u8,
    #[serde(default)]
    seasons: Vec<SeasonInfo>,
}

/// One season, as the service records where it stands.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeasonInfo {
    season_number: u32,
    #[serde(default)]
    status: u8,
}

/// One title in full: a film's or a series' fields, each absent from the other.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Full {
    #[serde(default)]
    overview: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    first_air_date: Option<String>,
    #[serde(default)]
    releases: Option<Releases>,
    #[serde(default)]
    content_ratings: Option<Ratings>,
    #[serde(default)]
    seasons: Vec<SeasonNumber>,
    #[serde(default)]
    media_info: Option<MediaInfo>,
}

/// A film's release dates, per region.
#[derive(Deserialize)]
struct Releases {
    #[serde(default)]
    results: Vec<RegionReleases>,
}

/// One region's release dates, each with the certification it was released under.
#[derive(Deserialize)]
struct RegionReleases {
    iso_3166_1: String,
    #[serde(default)]
    release_dates: Vec<ReleaseDate>,
}

/// One release date's certification.
#[derive(Deserialize)]
struct ReleaseDate {
    #[serde(default)]
    certification: String,
}

/// A series' content ratings, per region.
#[derive(Deserialize)]
struct Ratings {
    #[serde(default)]
    results: Vec<Rating>,
}

/// One region's rating.
#[derive(Deserialize)]
struct Rating {
    iso_3166_1: String,
    #[serde(default)]
    rating: String,
}

/// One season's number.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeasonNumber {
    season_number: u32,
}

/// A request to file: the title, the member it is filed for, and for a series the seasons
/// of it, every one where none are named.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Filing {
    media_type: &'static str,
    media_id: u64,
    user_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    seasons: Option<serde_json::Value>,
}

/// A filed request: its number and where it stands.
#[derive(Deserialize)]
struct Filed {
    id: i64,
    #[serde(default)]
    status: u8,
}

/// The number the service names a title or a member by, or nothing for any other text.
fn number(id: &str) -> Option<u64> {
    id.parse().ok()
}

/// The year a date opens with.
fn year_of(date: Option<&str>) -> Option<u16> {
    date?.get(..4)?.parse().ok()
}

/// Where a title stands, unknown where the service holds nothing of it.
fn standing(info: Option<&MediaInfo>) -> MediaStatus {
    info.and_then(|info| media_status(info.status))
        .unwrap_or(MediaStatus::Unknown)
}

impl Hit {
    /// The title this is, where it is a film or a series of one of `kinds`.
    fn found(self, kinds: &[Kind]) -> Option<Found> {
        let kind = kind_of(&self.media_type).filter(|kind| kinds.contains(kind))?;
        Some(Found {
            id: self.id.to_string(),
            kind,
            title: self.title.or(self.name)?,
            year: year_of(self.release_date.or(self.first_air_date).as_deref()),
            status: standing(self.media_info.as_ref()),
            poster: self.poster_path.map(|path| format!("{POSTERS}{path}")),
        })
    }
}

impl Full {
    /// What this title is, with its certification in `region`.
    fn detail(self, kind: Kind, region: &str) -> Detail {
        let certification = match kind {
            Kind::Movies => self.releases.and_then(|releases| {
                releases
                    .results
                    .into_iter()
                    .find(|one| one.iso_3166_1.eq_ignore_ascii_case(region))?
                    .release_dates
                    .into_iter()
                    .map(|date| date.certification)
                    .find(|certification| !certification.is_empty())
            }),
            Kind::Tv => self.content_ratings.and_then(|ratings| {
                ratings
                    .results
                    .into_iter()
                    .find(|one| one.iso_3166_1.eq_ignore_ascii_case(region))
                    .map(|one| one.rating)
                    .filter(|rating| !rating.is_empty())
            }),
        };
        let status = self.status.as_deref().unwrap_or_default();
        let released = match kind {
            Kind::Movies => status == RELEASED,
            Kind::Tv => self.first_air_date.is_some() && !UNRELEASED_SERIES.contains(&status),
        };
        let held = self.media_info.unwrap_or_default().seasons;
        let seasons = self
            .seasons
            .into_iter()
            .map(|season| season.season_number)
            .filter(|number| *number != SPECIALS)
            .map(|number| Season {
                number,
                status: held
                    .iter()
                    .find(|one| one.season_number == number)
                    .and_then(|one| media_status(one.status))
                    .unwrap_or(MediaStatus::Unknown),
            })
            .collect();
        Detail {
            overview: self.overview.filter(|overview| !overview.is_empty()),
            certification,
            released,
            seasons,
        }
    }
}

#[async_trait]
impl Searching for Seerr {
    async fn search(&self, term: &str, kinds: &[Kind], page: u32) -> Result<Page, Failure> {
        let path = format!("/search?query={}&page={page}", query_encoded(term));
        let response = self
            .endpoint
            .send(&self.request(Method::Get, &path, None))
            .await?;
        let read: SearchPage = self
            .endpoint
            .decode(&response, "the search could not be read")?;
        Ok(Page {
            next: (read.page < read.total_pages).then_some(read.page + 1),
            titles: read
                .results
                .into_iter()
                .filter_map(|one| one.found(kinds))
                .collect(),
        })
    }

    async fn detail(&self, kind: Kind, id: &str, region: &str) -> Result<Option<Detail>, Failure> {
        detail(self, kind, id, region).await
    }

    async fn ask(&self, member: &str, wish: &Wish) -> Result<Asked, Failure> {
        ask(self, member, wish).await
    }
}

async fn detail(
    seerr: &Seerr,
    kind: Kind,
    id: &str,
    region: &str,
) -> Result<Option<Detail>, Failure> {
    let Some(id) = number(id) else {
        return Ok(None);
    };
    let path = format!("/{}/{id}", media_type(kind));
    let response = seerr
        .endpoint
        .send(&seerr.request(Method::Get, &path, None))
        .await?;
    if response.status == NOT_FOUND {
        return Ok(None);
    }
    let full: Full = seerr
        .endpoint
        .decode(&response, "the title could not be read")?;
    Ok(Some(full.detail(kind, region)))
}

async fn ask(seerr: &Seerr, member: &str, wish: &Wish) -> Result<Asked, Failure> {
    let (Some(media), Some(user)) = (number(&wish.id), number(member)) else {
        return Err(seerr
            .endpoint
            .refused("the title or the member is not one the service names"));
    };
    let filing = Filing {
        media_type: media_type(wish.kind),
        media_id: media,
        user_id: user,
        seasons: (wish.kind == Kind::Tv).then(|| {
            if wish.seasons.is_empty() {
                serde_json::json!("all")
            } else {
                serde_json::json!(wish.seasons)
            }
        }),
    };
    let body = serde_json::to_string(&filing).unwrap_or_default();
    let response = seerr
        .endpoint
        .send(&seerr.request(Method::Post, "/request", Some(body)))
        .await?;
    let filed: Filed = seerr
        .endpoint
        .decode(&response, "the request could not be filed")?;
    Ok(Asked {
        request: filed.id,
        waiting: request_status(filed.status) == Some(RequestStatus::Pending),
    })
}

#[cfg(test)]
mod tests;
