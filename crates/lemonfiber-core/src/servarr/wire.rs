//! What a \*arr's JSON looks like on the wire.
//!
//! The shapes only, and deliberately nothing else: these types say what a service
//! sends and what it is sent, and every judgement about what it *means* — whether
//! a queue item counts as stuck, whether a client is the one we are looking for —
//! belongs to the code that reads them.
//!
//! Apart from the client for the same reason the client is apart from the policy.
//! A field a service added is a change here and nowhere else, and three modules
//! read these — the wiring writes, the queue and the trace — so they cannot each
//! keep their own idea of the shape.

use serde::Deserialize;

use crate::ports::service::{ClientProbe, RegisteredClient};

/// One group of complaints about a queue item.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StatusMessage {
    #[serde(default)]
    pub(super) messages: Vec<String>,
}

/// A page of the queue as the service reports it: its own total, and the records
/// on this page whose statuses the stuck count is read from.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct QueueResource {
    #[serde(default)]
    pub(super) total_records: usize,
    #[serde(default)]
    pub(super) records: Vec<QueueRecord>,
}

/// One queued item — its tracked download status (for the dashboard's stuck count) and,
/// for a per-item trace, which item it belongs to and how far its download has got. When
/// the queue is read with the series or movie included, it also carries the item's own
/// title, so a stuck item names the show a trace would search by.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct QueueRecord {
    /// What the service calls it — the name the download client knows it by too.
    #[serde(default)]
    pub(super) title: Option<String>,
    /// What the service said went wrong, where it reported a single message.
    #[serde(default)]
    pub(super) error_message: Option<String>,
    /// Where an import failure explains itself: the service groups its complaints
    /// under a title each, and the messages beneath are the operator-facing words.
    #[serde(default)]
    pub(super) status_messages: Vec<StatusMessage>,
    /// The download client's own identifier, which correlates the two sides more
    /// surely than a title either may have rewritten.
    #[serde(default)]
    pub(super) download_id: Option<String>,
    #[serde(default)]
    pub(super) tracked_download_status: String,
    #[serde(default)]
    pub(super) tracked_download_state: String,
    #[serde(default)]
    pub(super) series_id: Option<i64>,
    #[serde(default)]
    pub(super) movie_id: Option<i64>,
    /// The episode this record is for, where the service files its queue per episode.
    #[serde(default)]
    pub(super) episode_id: Option<i64>,
    /// The series this record belongs to, where the queue was read with it included.
    #[serde(default)]
    pub(super) series: Option<TitledResource>,
    /// The movie this record belongs to, where the queue was read with it included.
    #[serde(default)]
    pub(super) movie: Option<TitledResource>,
}

/// Just the title of a series or movie the queue embeds, for naming a stuck item.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TitledResource {
    #[serde(default)]
    pub(super) title: String,
}

impl QueueRecord {
    /// The item this record is for, whichever kind of service filed it — the
    /// episode where the queue is filed per episode, the film otherwise. What a
    /// re-grab is counted against, since a loop commonly changes release while
    /// staying the same item.
    pub(super) const fn item(&self) -> Option<i64> {
        match self.episode_id {
            Some(episode) => Some(episode),
            None => self.movie_id,
        }
    }

    /// Whether this record is for the given item — matched on the id field the item's
    /// kind files under.
    pub(super) fn is_for(&self, kind: crate::recyclarr::Kind, id: i64) -> bool {
        let owner = match kind {
            crate::recyclarr::Kind::Sonarr => self.series_id,
            crate::recyclarr::Kind::Radarr => self.movie_id,
        };
        owner == Some(id)
    }

    /// The item's title a trace would search by — the embedded series or movie title for
    /// the record's kind. Nothing where none was included: a queue item the \*arr has not
    /// matched to a library item is one a trace could not find, so it is left out of a
    /// stuck list rather than linked to a search that would come back empty.
    pub(super) fn item_title(&self, kind: crate::recyclarr::Kind) -> Option<String> {
        match kind {
            crate::recyclarr::Kind::Sonarr => self.series.as_ref(),
            crate::recyclarr::Kind::Radarr => self.movie.as_ref(),
        }
        .map(|resource| resource.title.clone())
        .filter(|title| !title.is_empty())
    }
}

/// A download-client resource as the service reports it: the identifier it
/// assigned, and the connection settings, which Servarr carries as named entries
/// in a `fields` array rather than as top-level keys.
#[derive(Deserialize)]
pub(super) struct ClientResource {
    pub(super) id: i64,
    #[serde(default)]
    pub(super) fields: Vec<ClientField>,
}

/// One entry in a resource's `fields` array — a setting's name and its value,
/// whose type varies by setting, so it is read as untyped JSON and interpreted
/// per field.
#[derive(Deserialize)]
pub(super) struct ClientField {
    pub(super) name: String,
    #[serde(default)]
    pub(super) value: serde_json::Value,
}

impl ClientResource {
    /// The client as the endpoint it reaches, or nothing where it does not name
    /// both a host and a port — which is all that a connection can be matched by,
    /// so one that names neither cannot be told from another and is left out.
    pub(super) fn endpoint(self) -> Option<RegisteredClient> {
        let host = self.field("host")?.as_str()?.to_owned();
        let port = u16::try_from(self.field("port")?.as_u64()?).ok()?;
        Some(RegisteredClient {
            id: self.id.to_string(),
            host,
            port,
            category: self.category(),
        })
    }

    /// The category the client files under, read from whichever `*Category` field
    /// the target application names it — `tvCategory`, `movieCategory`,
    /// `musicCategory`. Nothing where the client carries no such field.
    pub(super) fn category(&self) -> Option<crate::ports::service::Category> {
        let field = self
            .fields
            .iter()
            .find(|field| field.name.ends_with("Category"))?;
        Some(crate::ports::service::Category {
            field: field.name.clone(),
            value: field.value.as_str()?.to_owned(),
        })
    }

    /// The value of the named field, where the resource carries it.
    pub(super) fn field(&self, name: &str) -> Option<&serde_json::Value> {
        self.fields
            .iter()
            .find(|field| field.name == name)
            .map(|field| &field.value)
    }
}

/// The fields of a root-folder resource that seed reads back: the identifier the
/// service assigned, and the path, to match a wanted folder against.
#[derive(Deserialize)]
pub(super) struct FolderResource {
    pub(super) id: i64,
    pub(super) path: String,
}

/// One entry in a `testall` response: the id of the client tested, whether it
/// validated, and — where it did not — the failure messages the service gave.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TestResource {
    pub(super) id: i64,
    pub(super) is_valid: bool,
    #[serde(default)]
    pub(super) validation_failures: Vec<TestFailure>,
}

/// One failure in a test result — the service's own words for why a client did
/// not answer, joined into the reason a warning names.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TestFailure {
    #[serde(default)]
    pub(super) error_message: String,
}

impl TestResource {
    /// The test result as a [`ClientProbe`]: reachable where it validated, and — where
    /// it did not — the joined failure messages as the detail, or nothing where the
    /// service failed it without saying why.
    pub(super) fn probe(self) -> ClientProbe {
        let detail = if self.is_valid {
            None
        } else {
            let joined = self
                .validation_failures
                .into_iter()
                .map(|failure| failure.error_message)
                .filter(|message| !message.is_empty())
                .collect::<Vec<_>>()
                .join("; ");
            (!joined.is_empty()).then_some(joined)
        };
        ClientProbe {
            id: self.id.to_string(),
            reachable: self.is_valid,
            detail,
        }
    }
}
