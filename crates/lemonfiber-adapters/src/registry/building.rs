//! The build provenance an image builder lists in an image index beside the images it
//! built: one attestation manifest for each, whose layers are in-toto statements.
//!
//! Every count and size is read off what the index and its attestations declare before
//! anything they name is fetched, and each fetch holds no more than its descriptor
//! declared. A
//! registry listing more than [`MOST_ENTRIES`], [`MOST_STATEMENTS`], [`LARGEST_MANIFEST`],
//! [`LARGEST_STATEMENT`] or [`MOST_STATEMENT_BYTES`] allow is answered as beyond them, with
//! no statement read.

use std::collections::BTreeSet;

use lemonfiber_ports::http::Http;
use lemonfiber_ports::registry::{Built, Image, Statement, Unanswerable};

use super::served::{by_digest, json_by_digest};
use super::{addressed, runs, ANYTHING, INDEXES, MANIFESTS};

/// The most entries of an image index read.
pub(crate) const MOST_ENTRIES: usize = 64;

/// The most build provenance statements read off one attestation manifest.
pub(crate) const MOST_STATEMENTS: usize = 4;

/// The largest attestation manifest read, in bytes.
pub(crate) const LARGEST_MANIFEST: usize = 64 * 1024;

/// The largest build provenance statement read, in bytes.
pub(crate) const LARGEST_STATEMENT: usize = 1024 * 1024;

/// The most bytes of build provenance statements read for one image.
pub(crate) const MOST_STATEMENT_BYTES: usize = 4 * 1024 * 1024;

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

/// Why a read stopped: the registry could not be read, or listed more than a read holds.
enum Stopped {
    Unreadable(String),
    Beyond(String),
}

/// The build provenance the registry lists beside `image`'s platforms' images, asked over
/// one transport.
pub(super) async fn built(reaching: &dyn Http, image: &Image) -> Result<Built, Unanswerable> {
    let mut built = Built::default();
    match read(reaching, image, &mut built).await {
        Ok(()) => Ok(built),
        Err(Stopped::Unreadable(why)) => Err(Unanswerable::about(image, &why)),
        Err(Stopped::Beyond(why)) => Ok(Built {
            images: built.images,
            statements: Vec::new(),
            beyond: Some(why),
        }),
    }
}

/// Every image and statement the registry lists beside `image`, gathered into `built`.
async fn read(reaching: &dyn Http, image: &Image, built: &mut Built) -> Result<(), Stopped> {
    let (host, path) = addressed(&image.repository);
    let mut reading = Reading {
        host: &host,
        path: &path,
        held: None,
        read: BTreeSet::new(),
        bytes: 0,
    };
    let index = json_by_digest(
        reaching,
        (
            &host,
            &format!("https://{host}/v2/{path}/manifests/{}", image.digest),
            INDEXES,
        ),
        (&image.digest, None),
        &mut reading.held,
    )
    .await
    .map_err(Stopped::Unreadable)?;
    let Some(listed) = index.get("manifests").and_then(serde_json::Value::as_array) else {
        built.images.push(image.digest.clone());
        return Ok(());
    };
    if listed.len() > MOST_ENTRIES {
        return Err(Stopped::Beyond(format!(
            "its index lists {} entries, and a read holds {MOST_ENTRIES}",
            listed.len()
        )));
    }
    let mut attested = BTreeSet::new();
    for entry in listed {
        let digest = text(entry, &["digest"]).ok_or_else(|| {
            Stopped::Unreadable("an image its index lists names no digest".to_owned())
        })?;
        if runs(entry) {
            built.images.push(digest.to_owned());
            continue;
        }
        let Some(attests) = text(entry, &["annotations", ATTESTS])
            .filter(|_| text(entry, &["annotations", KIND]) == Some(ATTESTATION))
        else {
            continue;
        };
        if !attested.insert(digest) {
            continue;
        }
        statements(reaching, &mut reading, (entry, digest, attests), built).await?;
    }
    Ok(())
}

/// Where a read of one image's provenance asks, and what it has read so far.
struct Reading<'a> {
    host: &'a str,
    path: &'a str,
    held: Option<String>,
    read: BTreeSet<String>,
    bytes: usize,
}

/// Every statement the attestation `entry` lists, at `digest` and attesting `attests`,
/// gathered into `built`.
async fn statements(
    reaching: &dyn Http,
    reading: &mut Reading<'_>,
    (entry, digest, attests): (&serde_json::Value, &str, &str),
    built: &mut Built,
) -> Result<(), Stopped> {
    let declared = within(
        entry,
        LARGEST_MANIFEST,
        &format!("its attestation {digest}"),
    )?;
    let attestation = json_by_digest(
        reaching,
        (
            reading.host,
            &format!(
                "https://{}/v2/{}/manifests/{digest}",
                reading.host, reading.path
            ),
            MANIFESTS,
        ),
        (digest, Some(declared)),
        &mut reading.held,
    )
    .await
    .map_err(Stopped::Unreadable)?;
    let layers: Vec<&serde_json::Value> = attestation
        .get("layers")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| Stopped::Unreadable(format!("its attestation {digest} names no layers")))?
        .iter()
        .filter(|layer| {
            text(layer, &["annotations", PREDICATE])
                .is_some_and(|kind| kind.starts_with(PROVENANCE))
        })
        .collect();
    if layers.len() > MOST_STATEMENTS {
        return Err(Stopped::Beyond(format!(
            "its attestation {digest} lists {} provenance statements, and a read holds \
             {MOST_STATEMENTS}",
            layers.len()
        )));
    }
    for layer in layers {
        let blob = text(layer, &["digest"]).ok_or_else(|| {
            Stopped::Unreadable(format!(
                "a statement its attestation {digest} lists names no blob"
            ))
        })?;
        if !reading.read.insert(blob.to_owned()) {
            continue;
        }
        let declared = within(layer, LARGEST_STATEMENT, &format!("the statement {blob}"))?;
        reading.bytes += declared;
        if reading.bytes > MOST_STATEMENT_BYTES {
            return Err(Stopped::Beyond(format!(
                "its statements come to more than the {MOST_STATEMENT_BYTES} bytes a read \
                 holds"
            )));
        }
        let body = by_digest(
            reaching,
            (
                reading.host,
                &format!("https://{}/v2/{}/blobs/{blob}", reading.host, reading.path),
                ANYTHING,
            ),
            (blob, Some(declared)),
            &mut reading.held,
        )
        .await
        .map_err(Stopped::Unreadable)?;
        built.statements.push(Statement {
            attests: attests.to_owned(),
            body,
        });
    }
    Ok(())
}

/// The size `descriptor` declares, where it declares one no larger than `largest`.
fn within(descriptor: &serde_json::Value, largest: usize, named: &str) -> Result<usize, Stopped> {
    let size = descriptor
        .get("size")
        .and_then(serde_json::Value::as_u64)
        .and_then(|size| usize::try_from(size).ok())
        .ok_or_else(|| Stopped::Unreadable(format!("{named} declares no size")))?;
    if size > largest {
        return Err(Stopped::Beyond(format!(
            "{named} declares {size} bytes, and a read holds {largest}"
        )));
    }
    Ok(size)
}

/// The text `value` holds at `path`, one key at a time.
fn text<'a>(value: &'a serde_json::Value, path: &[&str]) -> Option<&'a str> {
    path.iter()
        .try_fold(value, |at, key| at.get(key))
        .and_then(serde_json::Value::as_str)
}

#[cfg(test)]
mod tests;
