//! The quality reads and writes a Servarr-shape service answers — applying an audio
//! format to Lidarr's profiles, and judging a resolution preset against what the
//! indexers actually carry. Built on the same [`Servarr`](super::Servarr) client as the
//! provisioning adapter; the quality concern lives apart from it so each grows on its own.

use async_trait::async_trait;
use serde::Deserialize;

use super::Servarr;
use crate::ports::http::Method;
use crate::ports::service::{Failure, MusicQuality, QualityReleases, ReleaseProbe};

#[async_trait]
impl MusicQuality for Servarr {
    async fn apply_music_format(&self, format: crate::audio::Format) -> Result<(), Failure> {
        let prefer = crate::lidarr::prefers_hi_res(format);
        // A hi-res choice prefers 24-bit through a custom format; ensure it exists before
        // scoring it, so a profile has something its cutoff score can point at.
        if prefer {
            self.ensure_hi_res_format().await?;
        }
        let response = self
            .probe(&self.request(Method::Get, "/qualityprofile", None))
            .await?;
        let profiles: Vec<serde_json::Value> = self
            .endpoint
            .decode(&response, "the quality profiles could not be read")?;
        for profile in profiles {
            let text = serde_json::to_string(&profile).unwrap_or_default();
            let Some(rewritten) = crate::lidarr::rewrite(&text, format) else {
                continue;
            };
            let body =
                crate::lidarr::set_hi_res_preference(&rewritten, prefer).unwrap_or(rewritten);
            let Some(id) = profile.get("id").and_then(serde_json::Value::as_i64) else {
                continue;
            };
            let updated = self
                .probe(&self.request(Method::Put, &format!("/qualityprofile/{id}"), Some(body)))
                .await?;
            self.endpoint.expect_success(&updated)?;
        }
        Ok(())
    }
}

impl Servarr {
    /// Create the 24-bit custom format if the service does not already carry it, matched
    /// by name so a second run does not add a duplicate.
    async fn ensure_hi_res_format(&self) -> Result<(), Failure> {
        let response = self
            .probe(&self.request(Method::Get, "/customformat", None))
            .await?;
        let formats: Vec<serde_json::Value> = self
            .endpoint
            .decode(&response, "the custom formats could not be read")?;
        let present = formats.iter().any(|entry| {
            entry.get("name").and_then(serde_json::Value::as_str)
                == Some(crate::lidarr::HI_RES_FORMAT)
        });
        if present {
            return Ok(());
        }
        let created = self
            .probe(&self.request(
                Method::Post,
                "/customformat",
                Some(crate::lidarr::hi_res_custom_format()),
            ))
            .await?;
        self.endpoint.expect_success(&created)
    }
}

#[async_trait]
impl QualityReleases for Servarr {
    async fn probe_releases(&self, id_param: &str) -> Result<ReleaseProbe, Failure> {
        // One wanted item is enough to judge the preset against — search for what the
        // operator is actually missing rather than a made-up query.
        let response = self
            .probe(&self.request(Method::Get, "/wanted/missing?page=1&pageSize=1", None))
            .await?;
        let wanted: Wanted = self
            .endpoint
            .decode(&response, "the wanted list could not be read")?;
        let Some(record) = wanted.records.first() else {
            return Ok(ReleaseProbe::NothingWanted);
        };

        // A manual search hits the indexers live — the disruptive part the caller gates.
        let response = self
            .probe(&self.request(
                Method::Get,
                &format!("/release?{id_param}={}", record.id),
                None,
            ))
            .await?;
        let releases: Vec<ReleaseResource> = self
            .endpoint
            .decode(&response, "the release search could not be read")?;
        if releases.is_empty() {
            return Ok(ReleaseProbe::NoneFound);
        }
        // A release the service left unrejected is one its profile — the quality preset
        // included — would grab; if every release carries a rejection, the preset wants
        // none of what is out there.
        let matching = releases.iter().any(|release| release.rejections.is_empty());
        Ok(if matching {
            ReleaseProbe::Matching
        } else {
            ReleaseProbe::NoneMatch
        })
    }
}

/// A page of the wanted/missing list — only the first record's id is needed, to name
/// what a release search is for.
#[derive(Deserialize)]
struct Wanted {
    #[serde(default)]
    records: Vec<WantedRecord>,
}

/// One wanted item: its id, which is the episode (Sonarr) or movie (Radarr) a manual
/// search is run for.
#[derive(Deserialize)]
struct WantedRecord {
    id: i64,
}

/// One release a manual search returned — only whether the service's profile rejected
/// it matters here: an empty list means the profile, quality preset included, would
/// grab it.
#[derive(Deserialize)]
struct ReleaseResource {
    #[serde(default)]
    rejections: Vec<String>,
}
