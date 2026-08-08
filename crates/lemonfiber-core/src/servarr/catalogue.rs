//! Asking a \*arr to take something on — the one write in this shape that acquires
//! content rather than wiring services together.
//!
//! Four requests, in the order a walkthrough makes them: what do you know by this name,
//! where would you file it and to what standard, take it on and go and look, and — asked
//! first of all — have you got anything to look *with*.

use async_trait::async_trait;
use serde::Deserialize;

use super::Servarr;
use crate::ports::http::Method;
use crate::ports::service::{AddPlan, Added, Catalogue, CatalogueEntry, Failure};
use crate::recyclarr::Kind;

#[async_trait]
impl Catalogue for Servarr {
    async fn lookup(&self, kind: Kind, term: &str) -> Result<Vec<CatalogueEntry>, Failure> {
        // The term is whatever the operator typed, so it is encoded rather than pasted:
        // a title with a space or an ampersand in it is the ordinary case, not the edge.
        let query: String = form_urlencoded::Serializer::new(String::new())
            .append_pair("term", term)
            .finish();
        let path = format!("/{}/lookup?{query}", kind.library_endpoint());
        let response = self.probe(&self.request(Method::Get, &path, None)).await?;
        let results: Vec<LookupResult> = self
            .endpoint
            .decode(&response, "the catalogue could not be read")?;
        Ok(results
            .into_iter()
            .map(|result| result.entry(kind))
            .collect())
    }

    async fn add_plan(&self, kind: Kind) -> Result<AddPlan, Failure> {
        let folders = self
            .read::<RootFolderResource>("/rootfolder", "no root folder")
            .await?;
        let profiles = self
            .read::<ProfileResource>("/qualityprofile", "no quality profile")
            .await?;
        // The first of each, because that is what setup wired: a stack with several is
        // one the operator has arranged themselves, and the walkthrough's first item
        // belongs wherever the rest of the library is rather than somewhere of its own.
        let root_folder = folders.into_iter().map(|folder| folder.path).next();
        let quality_profile = profiles.into_iter().map(|profile| profile.id).next();
        match root_folder.zip(quality_profile) {
            Some((root_folder, quality_profile)) => Ok(AddPlan {
                root_folder,
                quality_profile,
            }),
            // Not a transport failure but a stack that was never finished, and it is
            // reported as such rather than as the service refusing something.
            None => Err(self.endpoint.unsupported(&format!(
                "{} has no root folder or no quality profile configured yet",
                kind.noun()
            ))),
        }
    }

    async fn add(
        &self,
        kind: Kind,
        entry: &CatalogueEntry,
        plan: &AddPlan,
    ) -> Result<Added, Failure> {
        let mut body = serde_json::json!({
            "title": entry.title,
            "qualityProfileId": plan.quality_profile,
            "rootFolderPath": plan.root_folder,
            "monitored": true,
            "addOptions": { kind.search_option(): true },
        });
        // The external identifier and the two fields only one of the services takes are
        // set by kind rather than sent to both: a field a service does not know is a
        // field it rejects the whole body over.
        if let Some(object) = body.as_object_mut() {
            object.insert(
                kind.reference_field().to_owned(),
                serde_json::json!(entry.reference),
            );
            match kind {
                Kind::Sonarr => object.insert("seasonFolder".to_owned(), serde_json::json!(true)),
                Kind::Radarr => object.insert(
                    "minimumAvailability".to_owned(),
                    serde_json::json!("released"),
                ),
            };
        }
        let path = format!("/{}", kind.library_endpoint());
        let response = self
            .probe(&self.request(Method::Post, &path, Some(body.to_string())))
            .await?;
        self.endpoint.expect_success(&response)?;
        let added: LookupResult = self
            .endpoint
            .decode(&response, "the service did not say what it took on")?;
        Ok(Added {
            id: added.id,
            title: added.title,
        })
    }

    async fn indexer_count(&self) -> Result<usize, Failure> {
        // Only the enabled ones: an indexer configured and switched off searches nothing,
        // and counting it would offer a walkthrough that must stop at the first step.
        Ok(self
            .read::<IndexerResource>("/indexer", "no indexer")
            .await?
            .into_iter()
            .filter(|indexer| indexer.enable_automatic_search || indexer.enable_interactive_search)
            .count())
    }
}

impl Servarr {
    /// Read a list from `path`, saying `what` where it cannot be read — the same shape
    /// three of the four catalogue requests have.
    async fn read<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        what: &str,
    ) -> Result<Vec<T>, Failure> {
        let response = self.probe(&self.request(Method::Get, path, None)).await?;
        self.endpoint
            .decode(&response, &format!("{what} could be read"))
    }
}

/// One catalogue result, as either service returns it. The identifier fields are both
/// optional because each service sends only its own, and the one that is present is used.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LookupResult {
    /// The service's own id — zero for something it does not hold, which is how the
    /// catalogue says "known to me, not mine".
    #[serde(default)]
    id: i64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    year: Option<u32>,
    #[serde(default)]
    tvdb_id: Option<i64>,
    #[serde(default)]
    tmdb_id: Option<i64>,
}

impl LookupResult {
    /// This result as the port's entry — the service's zero id read as "not held", which
    /// is the distinction a walkthrough must not re-acquire over.
    fn entry(self, kind: Kind) -> CatalogueEntry {
        CatalogueEntry {
            title: self.title,
            year: self.year,
            reference: match kind {
                Kind::Sonarr => self.tvdb_id,
                Kind::Radarr => self.tmdb_id,
            }
            .unwrap_or_default(),
            held_as: (self.id != 0).then_some(self.id),
        }
    }
}

/// The one field of a root folder that matters: where it is.
#[derive(Deserialize)]
struct RootFolderResource {
    #[serde(default)]
    path: String,
}

/// The one field of a quality profile that matters: its id, which an add refers to.
#[derive(Deserialize)]
struct ProfileResource {
    #[serde(default)]
    id: i64,
}

/// The two fields of an indexer that say whether it would ever be searched.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexerResource {
    #[serde(default)]
    enable_automatic_search: bool,
    #[serde(default)]
    enable_interactive_search: bool,
}

#[cfg(test)]
mod tests {
    use super::LookupResult;
    use crate::recyclarr::Kind;

    /// A catalogue result the way each service sends one.
    fn result(id: i64, television: Option<i64>, film: Option<i64>) -> LookupResult {
        LookupResult {
            id,
            title: "Sintel".to_owned(),
            year: Some(2010),
            tvdb_id: television,
            tmdb_id: film,
        }
    }

    #[test]
    fn each_service_is_read_by_its_own_identifier() {
        // Sonarr files by TVDB and Radarr by TMDB; reading the wrong one would produce
        // an entry that looks addable and is not.
        assert_eq!(result(0, Some(77), None).entry(Kind::Sonarr).reference, 77);
        assert_eq!(result(0, None, Some(99)).entry(Kind::Radarr).reference, 99);
        assert_eq!(
            result(0, Some(77), None).entry(Kind::Radarr).reference,
            0,
            "a television id is not a film id"
        );
    }

    #[test]
    fn a_zero_id_is_the_catalogue_saying_it_does_not_hold_this() {
        // The distinction the whole already-present detection rests on.
        assert!(!result(0, Some(1), None)
            .entry(Kind::Sonarr)
            .is_already_here());
        assert_eq!(
            result(42, Some(1), None).entry(Kind::Sonarr).held_as,
            Some(42)
        );
    }

    #[test]
    fn a_result_keeps_the_year_that_tells_two_of_a_name_apart() {
        let entry = result(0, Some(1), None).entry(Kind::Sonarr);
        assert_eq!(entry.named(), "Sintel (2010)");
    }
}
