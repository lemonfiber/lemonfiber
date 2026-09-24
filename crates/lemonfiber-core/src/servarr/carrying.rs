//! Reading what a service holds, and re-creating it on another.
//!
//! The whole of importing an operator's own records. Each kind is a path under the same
//! API answering with a list of objects, so one implementation serves all four.
//!
//! What is read is kept **as the service said it**, with only the two things that cannot
//! travel resolved out of it: the quality profile and the root folder. Both are numbered
//! per stack, so a record copied with its ids intact would point at whatever happened to
//! be third on the other machine — which is how an import ruins a library rather than
//! failing to copy it. They travel as a name and a path, and are looked up again at the
//! far end.

use async_trait::async_trait;
use serde_json::{Map, Value};

use crate::ports::http::Method;
use crate::ports::service::{Carried, Carrying, Failure, Record};

use super::Servarr;

/// The fields that cannot travel between two stacks.
const NUMBERED: [&str; 2] = ["qualityProfileId", "rootFolderPath"];

/// What a record is called, whichever of the two words this kind uses for it.
fn named(item: &Map<String, Value>) -> String {
    for key in ["title", "name", "artistName"] {
        if let Some(Value::String(said)) = item.get(key) {
            return said.clone();
        }
    }
    String::new()
}

#[async_trait]
impl Carrying for Servarr {
    async fn records(&self, kind: Record) -> Result<Vec<Carried>, Failure> {
        let held: Vec<Value> = self
            .read(
                kind.path(),
                &format!("the {} this service holds", kind.plural()),
            )
            .await?;

        let profiles = self.numbered_profiles().await?;

        Ok(held
            .into_iter()
            .filter_map(|item| item.as_object().cloned())
            .map(|item| Carried {
                name: named(&item),
                profile: item
                    .get("qualityProfileId")
                    .and_then(Value::as_u64)
                    .and_then(|id| profiles.get(&id).cloned()),
                folder: item
                    .get("rootFolderPath")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                rest: Value::Object(item).to_string(),
            })
            .filter(|carried| !carried.name.is_empty())
            .collect())
    }

    async fn carry(&self, kind: Record, item: &Carried) -> Result<(), Failure> {
        carry(self, kind, item).await
    }
}

impl Servarr {
    /// This service's quality profiles, by the id it gave each.
    async fn numbered_profiles(&self) -> Result<std::collections::BTreeMap<u64, String>, Failure> {
        let held: Vec<super::catalogue::ProfileResource> =
            self.read("/qualityprofile", "the quality profiles").await?;
        Ok(held
            .into_iter()
            .filter(|profile| !profile.name.is_empty())
            .filter_map(|profile| u64::try_from(profile.id).ok().map(|id| (id, profile.name)))
            .collect())
    }

    /// The id this service gives the profile of that name.
    ///
    /// # Errors
    ///
    /// Where it has no profile of that name, which is a record that cannot be carried
    /// rather than one to guess a profile for.
    async fn profile_named(&self, wanted: &str) -> Result<u64, Failure> {
        self.numbered_profiles()
            .await?
            .into_iter()
            .find(|(_, name)| name == wanted)
            .map(|(id, _)| id)
            .ok_or_else(|| Failure::Unsupported {
                service: self.service().to_owned(),
                detail: format!("no quality profile called {wanted}"),
            })
    }
}

async fn carry(servarr: &Servarr, kind: Record, item: &Carried) -> Result<(), Failure> {
    let Ok(Value::Object(mut body)) = serde_json::from_str::<Value>(&item.rest) else {
        return Err(Failure::Unsupported {
            service: servarr.service().to_owned(),
            detail: format!("a {} record that could not be read back", kind.plural()),
        });
    };

    // Its id here is not its id there, and a body carrying the old one asks this
    // service to replace a record it has never seen.
    body.remove("id");
    for field in NUMBERED {
        body.remove(field);
    }

    if let Some(wanted) = &item.profile {
        let id = servarr.profile_named(wanted).await?;
        body.insert("qualityProfileId".to_owned(), Value::from(id));
    }
    if let Some(folder) = &item.folder {
        body.insert("rootFolderPath".to_owned(), Value::from(folder.clone()));
    }

    let asked = servarr.request(
        Method::Post,
        kind.path(),
        Some(Value::Object(body).to_string()),
    );
    // A service that would not take it is not a service that took it. Without this
    // a refusal comes back as a record carried, and the operator is told their
    // library crossed when it did not.
    let answered = servarr.probe(&asked).await?;
    servarr.expect_success(&answered)
}

#[cfg(test)]
mod tests;
