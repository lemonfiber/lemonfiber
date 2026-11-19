//! The build provenance an image builder lists in an image index beside the images it
//! built: one attestation manifest for each, whose layers are in-toto statements.

use lemonfiber_ports::http::Http;
use lemonfiber_ports::registry::{Built, Image, Statement, Unanswerable};

use super::served::{by_digest, json_by_digest};
use super::{addressed, runs, ANYTHING, INDEXES, MANIFESTS};

/// The annotation an index entry names what kind of reference it is in.
const KIND: &str = "vnd.docker.reference.type";

/// The kind an attestation manifest is.
const ATTESTATION: &str = "attestation-manifest";

/// The annotation an attestation manifest names the image it attests in.
const ATTESTS: &str = "vnd.docker.reference.digest";

/// The annotation a layer names its statement's predicate type in.
const PREDICATE: &str = "in-toto.io/predicate-type";

/// What every build provenance predicate type starts with.
const PROVENANCE: &str = "https://slsa.dev/provenance/";

/// The build provenance the registry lists beside `image`'s platforms' images, asked over
/// one transport.
pub(super) async fn built(reaching: &dyn Http, image: &Image) -> Result<Built, Unanswerable> {
    let (host, path) = addressed(&image.repository);
    let unanswerable = |why: String| Unanswerable::about(image, &why);
    let mut held = None;
    let manifests = format!("https://{host}/v2/{path}/manifests");
    let index = json_by_digest(
        reaching,
        (&host, &format!("{manifests}/{}", image.digest), INDEXES),
        &image.digest,
        &mut held,
    )
    .await
    .map_err(unanswerable)?;
    let Some(listed) = index.get("manifests").and_then(serde_json::Value::as_array) else {
        return Ok(Built {
            images: vec![image.digest.clone()],
            statements: Vec::new(),
        });
    };
    let mut built = Built::default();
    for entry in listed {
        let digest = text(entry, &["digest"])
            .ok_or_else(|| unanswerable("an image its index lists names no digest".to_owned()))?;
        if runs(entry) {
            built.images.push(digest.to_owned());
            continue;
        }
        let Some(attests) = text(entry, &["annotations", ATTESTS])
            .filter(|_| text(entry, &["annotations", KIND]) == Some(ATTESTATION))
        else {
            continue;
        };
        let attestation = json_by_digest(
            reaching,
            (&host, &format!("{manifests}/{digest}"), MANIFESTS),
            digest,
            &mut held,
        )
        .await
        .map_err(unanswerable)?;
        let layers = attestation
            .get("layers")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| unanswerable(format!("its attestation {digest} names no layers")))?;
        for layer in layers.iter().filter(|layer| {
            text(layer, &["annotations", PREDICATE])
                .is_some_and(|kind| kind.starts_with(PROVENANCE))
        }) {
            let blob = text(layer, &["digest"]).ok_or_else(|| {
                unanswerable(format!(
                    "a statement its attestation {digest} lists names no blob"
                ))
            })?;
            let body = by_digest(
                reaching,
                (
                    &host,
                    &format!("https://{host}/v2/{path}/blobs/{blob}"),
                    ANYTHING,
                ),
                blob,
                &mut held,
            )
            .await
            .map_err(unanswerable)?;
            built.statements.push(Statement {
                attests: attests.to_owned(),
                body,
            });
        }
    }
    Ok(built)
}

/// The text `value` holds at `path`, one key at a time.
fn text<'a>(value: &'a serde_json::Value, path: &[&str]) -> Option<&'a str> {
    path.iter()
        .try_fold(value, |at, key| at.get(key))
        .and_then(serde_json::Value::as_str)
}

#[cfg(test)]
mod tests;
