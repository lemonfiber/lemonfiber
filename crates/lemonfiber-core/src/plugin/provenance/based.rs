//! Whether the upstream image a service says its image is built on is one the build of
//! every platform's image started from, as the build provenance beside it records.

use lemonfiber_plugin::BuiltOn;
use lemonfiber_ports::registry::{familiar, Built, Unanswerable};
use serde::Serialize;

use crate::pass::decoded;

/// What a service says its image is built on, and what this build makes of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Based {
    /// The upstream image, as the manifest names it.
    pub image: String,
    /// The upstream's release, as the manifest names it.
    pub release: String,
    /// What the build provenance makes of it.
    #[serde(flatten)]
    pub held: Basis,
}

/// What the build provenance beside an image makes of the upstream it says it is built on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "basis", rename_all = "kebab-case")]
pub enum Basis {
    /// The build of every platform's image is recorded as starting from it.
    Recorded {
        /// The digest of the upstream image each build is recorded as starting from.
        digests: Vec<String>,
    },
    /// The build provenance could not be asked for.
    Unproven {
        /// What stopped it being asked for.
        why: String,
    },
    /// The build of some platform's image is not recorded as starting from it.
    Refused {
        /// What each such build is recorded as starting from.
        why: String,
    },
}

impl Basis {
    /// Whether this stops an install.
    #[must_use]
    pub const fn refuses(&self) -> bool {
        matches!(self, Self::Refused { .. })
    }
}

/// The purl type an image a build started from is recorded under.
const DOCKER: &str = "pkg:docker/";

/// What a build provenance statement records one image the build started from as.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Started {
    image: String,
    release: String,
    digest: String,
}

impl std::fmt::Display for Started {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {} at {}", self.image, self.release, self.digest)
    }
}

/// What the build provenance `answer` makes of `service` saying it is built on `built_on`.
#[must_use]
pub fn based(service: &str, built_on: &BuiltOn, answer: &Result<Built, Unanswerable>) -> Based {
    let held = match answer {
        Err(unanswerable) => Basis::Unproven {
            why: format!(
                "{unanswerable}; nothing about what {service} is built on follows from a \
                 question nobody could put"
            ),
        },
        Ok(built) => held(service, built_on, built),
    };
    Based {
        image: built_on.image.clone(),
        release: built_on.release.clone(),
        held,
    }
}

/// What `built`'s statements record each platform's build as starting from, held to
/// `built_on`.
fn held(service: &str, built_on: &BuiltOn, built: &Built) -> Basis {
    let claimed = familiar(&built_on.image);
    let mut digests = Vec::new();
    let mut faults = Vec::new();
    if built.images.is_empty() {
        faults.push("its index lists no image a platform runs".to_owned());
    }
    for image in &built.images {
        let mut started = Vec::new();
        let mut unreadable = Vec::new();
        for statement in built.statements.iter().filter(|one| &one.attests == image) {
            match recorded(&statement.body, image) {
                Ok(read) => started.extend(read),
                Err(why) => unreadable.push(why),
            }
        }
        if let Some(found) = started
            .iter()
            .find(|one| familiar(&one.image) == claimed && one.release == built_on.release)
        {
            digests.push(found.digest.clone());
            continue;
        }
        let records = if started.is_empty() {
            "no image the build started from".to_owned()
        } else {
            started
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<String>>()
                .join(", ")
        };
        let unread = if unreadable.is_empty() {
            String::new()
        } else {
            format!(" ({})", unreadable.join("; "))
        };
        faults.push(format!(
            "the build provenance of {image} records {records}{unread}"
        ));
    }
    if !faults.is_empty() {
        return Basis::Refused {
            why: format!(
                "{service} says it is built on {} {}, and {}",
                built_on.image,
                built_on.release,
                faults.join("; ")
            ),
        };
    }
    digests.sort();
    digests.dedup();
    Basis::Recorded { digests }
}

/// Every image the build provenance statement `body` records the build of `image` as
/// starting from, where it is a statement about `image`.
fn recorded(body: &str, image: &str) -> Result<Vec<Started>, String> {
    let read: serde_json::Value = serde_json::from_str(body)
        .map_err(|why| format!("a statement is not readable as one ({why})"))?;
    let hex = image.strip_prefix("sha256:").unwrap_or(image);
    let about = read
        .get("subject")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|subjects| {
            subjects.iter().any(|subject| {
                subject
                    .get("digest")
                    .and_then(|digest| digest.get("sha256"))
                    .and_then(serde_json::Value::as_str)
                    == Some(hex)
            })
        });
    if !about {
        return Err(format!(
            "a statement listed beside {image} is about another image"
        ));
    }
    let predicate = read.get("predicate");
    let listed = [
        predicate
            .and_then(|predicate| predicate.get("buildDefinition"))
            .and_then(|definition| definition.get("resolvedDependencies")),
        predicate.and_then(|predicate| predicate.get("materials")),
    ];
    Ok(listed
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_array)
        .flatten()
        .filter_map(started)
        .collect())
}

/// The image one dependency a statement lists is, where it is an image named by a
/// docker purl with one sha256 digest.
fn started(dependency: &serde_json::Value) -> Option<Started> {
    let uri = dependency
        .get("uri")
        .and_then(serde_json::Value::as_str)?
        .strip_prefix(DOCKER)?;
    let (named, qualifiers) = uri.split_once('?').unwrap_or((uri, ""));
    let (image, release) = named.rsplit_once('@')?;
    let digest = dependency
        .get("digest")
        .and_then(|digest| digest.get("sha256"))
        .and_then(serde_json::Value::as_str)
        .map(|hex| format!("sha256:{hex}"))
        .filter(|digest| lemonfiber_ports::registry::is_digest(digest))?;
    let qualified = qualifiers
        .split('&')
        .find_map(|pair| pair.strip_prefix("digest="))
        .map(decoded);
    if qualified.is_some_and(|qualified| qualified.as_deref() != Some(digest.as_str())) {
        return None;
    }
    Some(Started {
        image: decoded(image)?,
        release: decoded(release)?,
        digest,
    })
}

#[cfg(test)]
mod tests;
