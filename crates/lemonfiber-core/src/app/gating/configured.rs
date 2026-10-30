//! The stack's own request gate, told through the two files it reads in its configuration
//! directory: the routes it admits with the credential it presents on each, and the hashes
//! of the tokens it admits there.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_sidecar::gate::{
    Accepted, Credential, File, Kind, Outcome, Record, Tokens, Upstream, Upstreams,
};
use lemonfiber_sidecar::{Pins, TokenHash};

use super::{path, SERVICE};
use crate::ports::filesystem::{Beneath, FileSystem};
use crate::ports::media;
use crate::ports::service::{
    Admitted, Failure, GateEntry, GateOutcome, GateRecord, Gated, Gating, Target,
};

/// The stack's own request gate, beneath the project its configuration is mounted from.
pub(crate) struct Configured {
    /// The project root the stack's configuration directories sit under.
    project: PathBuf,
    /// What its files are read back through, beneath the directory the gate owns.
    files: Arc<dyn FileSystem>,
}

impl Configured {
    /// The gate under `project`, its files read back through `files`.
    pub(crate) fn at(project: &Path, files: Arc<dyn FileSystem>) -> Self {
        Self {
            project: project.to_path_buf(),
            files,
        }
    }

    /// The text of `file`, where it is there and the gate's own.
    async fn read(&self, file: File) -> Option<String> {
        let at = path(&self.project, file);
        crate::app::targets::read_owned(self.files.as_ref(), &at, crate::within::directory_of(&at))
            .await
    }

    /// What the record the gate keeps holds: nothing where it is not there, and a refusal
    /// where something other than a plain file of the gate's own directory is, which is
    /// never followed.
    async fn kept(&self) -> Result<Option<String>, Failure> {
        let at = path(&self.project, File::Record);
        match self
            .files
            .read_beneath(&at, crate::within::directory_of(&at))
            .await
        {
            Beneath::Read(text) => Ok(Some(text)),
            Beneath::Absent => Ok(None),
            Beneath::Escaped => Err(Failure::Refused {
                service: SERVICE.to_owned(),
                detail: "its record is not a file of its own".to_owned(),
            }),
        }
    }

    /// Write `text` to `file` where it differs from `held`.
    fn written(&self, file: File, held: Option<&str>, text: &str) -> Result<(), Failure> {
        if held == Some(text) {
            return Ok(());
        }
        crate::config::store::write(&path(&self.project, file), text).map_err(|failure| {
            Failure::Refused {
                service: SERVICE.to_owned(),
                detail: failure.to_string(),
            }
        })
    }
}

#[async_trait]
impl Gating for Configured {
    async fn admit(&self, routes: &[Gated]) -> Result<(), Failure> {
        let upstreams = Upstreams::of(routes.iter().map(upstream).collect::<Result<_, _>>()?);
        let tokens = Tokens::of(
            routes
                .iter()
                .map(|route| Accepted {
                    route: route.path.clone(),
                    tokens: route
                        .tokens
                        .iter()
                        .map(|hash| TokenHash::written(hash))
                        .collect(),
                })
                .collect(),
        );
        self.written(
            File::Upstreams,
            self.read(File::Upstreams).await.as_deref(),
            &upstreams.written(),
        )?;
        self.written(
            File::Tokens,
            self.read(File::Tokens).await.as_deref(),
            &tokens.written(),
        )
    }

    async fn admitted(&self) -> Result<Vec<Admitted>, Failure> {
        let upstreams = self
            .read(File::Upstreams)
            .await
            .and_then(|text| Upstreams::read(&text).ok())
            .map(|read| read.upstreams)
            .unwrap_or_default();
        let tokens = self
            .read(File::Tokens)
            .await
            .and_then(|text| Tokens::read(&text).ok())
            .map(|read| read.routes)
            .unwrap_or_default();
        Ok(upstreams
            .iter()
            .map(|upstream| {
                let accepted = tokens
                    .iter()
                    .find(|one| one.route == upstream.route)
                    .map(|one| {
                        one.tokens
                            .iter()
                            .map(|hash| hash.as_str().to_owned())
                            .collect()
                    })
                    .unwrap_or_default();
                Admitted::of(&Gated {
                    target: target(upstream.kind, &upstream.pins),
                    path: upstream.route.clone(),
                    address: upstream.address.clone(),
                    majors: upstream.majors.clone(),
                    credential: upstream.credential.reveal().to_owned(),
                    tokens: accepted,
                })
            })
            .collect())
    }

    async fn admit_tokens(&self, route: &str, tokens: &[String]) -> Result<(), Failure> {
        let held = self.read(File::Tokens).await;
        let mut routes = held
            .as_deref()
            .and_then(|text| Tokens::read(text).ok())
            .map(|read| read.routes)
            .unwrap_or_default();
        let accepted: Vec<_> = tokens.iter().map(|hash| TokenHash::written(hash)).collect();
        match routes.iter_mut().find(|one| one.route == route) {
            Some(one) => one.tokens = accepted,
            None => {
                return Err(Failure::Refused {
                    service: SERVICE.to_owned(),
                    detail: format!("it admits no route under {route}"),
                })
            }
        }
        self.written(File::Tokens, held.as_deref(), &Tokens::of(routes).written())
    }

    async fn record(&self, after: u64) -> Result<GateRecord, Failure> {
        let Some(text) = self.kept().await? else {
            return Ok(GateRecord::default());
        };
        let record = Record::read(&text).map_err(|unreadable| Failure::Refused {
            service: SERVICE.to_owned(),
            detail: format!("its record could not be read: {unreadable}"),
        })?;
        let (entries, lost) = record.since(after);
        Ok(GateRecord {
            entries: entries
                .into_iter()
                .map(|entry| GateEntry {
                    seq: entry.seq,
                    at: entry.at,
                    route: entry.route.clone(),
                    method: entry.method.clone(),
                    path: entry.path.clone(),
                    outcome: match entry.outcome {
                        Outcome::Refused => GateOutcome::Refused,
                        Outcome::Removed => GateOutcome::Removed,
                    },
                })
                .collect(),
            lost,
        })
    }
}

/// `route` as the gate's routes file holds it, where the gate speaks to what it reaches.
fn upstream(route: &Gated) -> Result<Upstream, Failure> {
    Ok(Upstream {
        route: route.path.clone(),
        kind: kind(&route.target)?,
        address: route.address.clone(),
        credential: Credential::new(route.credential.clone()),
        majors: route.majors.clone(),
        pins: match &route.target {
            Target::MediaServer { pins } => Pins::of(pins).map_err(|odd| Failure::Refused {
                service: SERVICE.to_owned(),
                detail: odd.why,
            })?,
            Target::Curator { .. } => Pins::default(),
        },
    })
}

/// What the gate calls `target`: a curator of video by the kind it fetches, and the media
/// server.
fn kind(target: &Target) -> Result<Kind, Failure> {
    match target {
        Target::Curator { media_type } => media::Kind::for_media_type(media_type)
            .map(super::route_for)
            .ok_or_else(|| Failure::Refused {
                service: SERVICE.to_owned(),
                detail: format!("it reaches no curator filing {media_type}"),
            }),
        Target::MediaServer { .. } => Ok(Kind::Jellyfin),
    }
}

/// What a route of `kind` reaches, the media server's by the certificates in `pins`.
fn target(kind: Kind, pins: &Pins) -> Target {
    super::fetching(kind).map_or_else(
        || Target::MediaServer {
            pins: pins.listed().to_vec(),
        },
        |fetching| Target::Curator {
            media_type: fetching.media_type().to_owned(),
        },
    )
}

#[cfg(test)]
mod tests;
