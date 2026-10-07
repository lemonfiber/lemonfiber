//! What the media server is playing now, read off its sessions.
//!
//! Its own file because it is a different question from the devices beside it. Those
//! are who is signed in where; this is who is watching what, which is the one fact
//! about the household most likely to have changed since anybody last asked.

use super::{item_type, Jellyfin, Kind, Medium, Method, SESSIONS};
use crate::ports::service::{Failure, Playback};

/// The word the media server files one episode of a series under.
const EPISODE: &str = "Episode";

/// One session as the media server describes it, with what it is playing.
#[derive(serde::Deserialize)]
struct PlayingResource {
    #[serde(rename = "UserId", default)]
    user_id: String,
    #[serde(rename = "UserName", default)]
    user: String,
    #[serde(rename = "DeviceName", default)]
    device: String,
    /// Absent on a session signed in and playing nothing.
    #[serde(rename = "NowPlayingItem", default)]
    item: Option<NowPlaying>,
    #[serde(rename = "PlayState", default)]
    state: PlayState,
}

/// What a session is playing.
#[derive(serde::Deserialize)]
struct NowPlaying {
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "SeriesName", default)]
    series: Option<String>,
    #[serde(rename = "ParentIndexNumber", default)]
    season: Option<u32>,
    #[serde(rename = "IndexNumber", default)]
    episode: Option<u32>,
    #[serde(rename = "Type", default)]
    medium: String,
}

/// Whether a session's playback is paused.
#[derive(serde::Deserialize, Default)]
struct PlayState {
    #[serde(rename = "IsPaused", default)]
    paused: bool,
}

/// What is playing now, for one account or for every account.
///
/// Beside the impl rather than inside it because it decides, and nothing inside an
/// `#[async_trait]` body is attributed to a line in the coverage report.
///
/// **Narrowed to one account here rather than by the server**, for the reason the
/// devices beside it are: the server's own narrowing is to the sessions an account may
/// control remotely, which for an administrator is every session in the house.
pub(super) async fn playing(
    jellyfin: &Jellyfin,
    member: Option<&str>,
) -> Result<Vec<Playback>, Failure> {
    let response = jellyfin.as_admin(Method::Get, SESSIONS, None).await?;
    let listed: Vec<PlayingResource> = jellyfin.endpoint.decode(
        &response,
        "what the media server is playing could not be read",
    )?;
    Ok(listed
        .into_iter()
        .filter(|session| member.is_none_or(|member| session.user_id == member))
        .filter_map(PlayingResource::playback)
        .collect())
}

/// Which of the kinds this product deals in a playing item is, from the server's word.
///
/// An episode is of a series. Read through [`item_type`] for the two the shelf reads,
/// so the words cannot drift from the ones the shelf asks for.
fn medium(word: &str) -> Medium {
    if word == item_type(Kind::Radarr) {
        Medium::Film
    } else if word == EPISODE || word == item_type(Kind::Sonarr) {
        Medium::Series
    } else {
        Medium::Other
    }
}

impl PlayingResource {
    /// The same session in this product's own words, or nothing where nothing plays.
    fn playback(self) -> Option<Playback> {
        let item = self.item?;
        Some(Playback {
            member_id: self.user_id,
            member: self.user,
            title: item.name,
            series: item.series,
            season: item.season,
            episode: item.episode,
            medium: medium(&item.medium),
            paused: self.state.paused,
            device: self.device,
        })
    }
}
