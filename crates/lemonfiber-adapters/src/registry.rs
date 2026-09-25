//! Asking an OCI registry what it holds beside an image.
//!
//! The convention this reads is the one the tooling that makes these signatures
//! writes: a signature for `…@sha256:<hex>` is an ordinary image in the same
//! repository, tagged `sha256-<hex>.sig`, whose layers each carry the signature in
//! an annotation and whose blob is the payload that was signed. Nothing about it is
//! lemonfiber's invention, and nothing here decides what any of it is worth.
//!
//! **A repository with no signature answers 404, and that is an answer.** It is the
//! one case that must not be reported as a failure to ask: a publisher who signed
//! nothing has said something, and turning it into an outage would make every
//! unsigned image indistinguishable from every unreachable one.
//!
//! Everything else that goes wrong is [`Unanswerable`]. A token this build does not
//! hold, a rate limit, an answer in a shape it cannot read — none of them says
//! anything about the image, and each is reported as the question not having been
//! put.

use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine as _;
use lemonfiber_ports::http::{Http, Method, Request};
use lemonfiber_ports::registry::{Image, Offered, Registry, Unanswerable};

/// Where the signature's own bytes sit on a layer.
const SIGNATURE: &str = "dev.cosignproject.cosign/signature";

/// What a registry is told this build will accept.
const MANIFESTS: &str = "application/vnd.oci.image.manifest.v1+json, \
                         application/vnd.docker.distribution.manifest.v2+json";

/// Reads an OCI registry over whatever speaks HTTP.
///
/// The transport arrives as a shared handle rather than by type. A generic here is a
/// second set of coverage counters for every instantiation the workspace makes, and
/// the one this binary builds in production is the one no test enters — which reads
/// as a body nothing covers while every line of it is exercised.
pub struct Oci {
    reaching: Arc<dyn Http>,
}

impl Oci {
    /// A registry reader over one transport.
    #[must_use]
    pub const fn new(reaching: Arc<dyn Http>) -> Self {
        Self { reaching }
    }
}

/// The registry host and the path within it, split out of a repository name.
///
/// `docker.io` is written `registry-1.docker.io` and its library images carry an
/// implicit `library/` — both are facts about that one registry rather than about
/// the format, and a reader that did not know them would ask the wrong host for
/// every image most operators actually run.
fn addressed(repository: &str) -> (String, String) {
    let (host, path) = repository
        .split_once('/')
        .filter(|(host, _)| host.contains('.') || host.contains(':') || *host == "localhost")
        .unwrap_or(("docker.io", repository));
    let host = if host == "docker.io" {
        "registry-1.docker.io"
    } else {
        host
    };
    let path = if host == "registry-1.docker.io" && !path.contains('/') {
        format!("library/{path}")
    } else {
        path.to_owned()
    };
    (host.to_owned(), path)
}

/// The tag a signature for this digest is published under.
fn beside(digest: &str) -> String {
    format!("{}.sig", digest.replacen(':', "-", 1))
}

#[async_trait]
impl Registry for Oci {
    async fn signatures(&self, image: &Image) -> Result<Vec<Offered>, Unanswerable> {
        asking(self.reaching.as_ref(), image).await
    }
}

/// What a registry holds beside one image, asked over one transport.
///
/// A plain function rather than the method's own body. `#[async_trait]` rewrites a
/// body into a generated future, and the coverage report attributes nothing inside it
/// to the lines it came from — so every refusal below would be a branch that could go
/// untaken for ever with the gate saying nothing. Here each one is watched.
async fn asking(reaching: &dyn Http, image: &Image) -> Result<Vec<Offered>, Unanswerable> {
    let (host, path) = addressed(&image.repository);
    let manifest = format!(
        "https://{host}/v2/{path}/manifests/{}",
        beside(&image.digest)
    );
    let answer = reaching
        .send(&Request {
            method: Method::Get,
            url: manifest.clone(),
            headers: vec![("Accept".to_owned(), MANIFESTS.to_owned())],
            body: None,
        })
        .await
        .map_err(|why| Unanswerable::about(image, &why.reason))?;

    // Nothing is signed here, which the registry has said rather than failed to.
    if answer.status == 404 {
        return Ok(Vec::new());
    }
    if !answer.is_success() {
        return Err(Unanswerable::about(
            image,
            &format!("{manifest} answered {}", answer.status),
        ));
    }

    let read: serde_json::Value = serde_json::from_str(&answer.body)
        .map_err(|why| Unanswerable::about(image, &format!("its manifest is not JSON: {why}")))?;
    let layers = read
        .get("layers")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| Unanswerable::about(image, "its manifest names no layers"))?;

    let mut offered = Vec::new();
    for layer in layers {
        let Some(signature) = layer
            .get("annotations")
            .and_then(|on| on.get(SIGNATURE))
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let signature = base64::engine::general_purpose::STANDARD
            .decode(signature)
            .map_err(|why| {
                Unanswerable::about(image, &format!("a signature is not base64: {why}"))
            })?;
        let digest = layer
            .get("digest")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| Unanswerable::about(image, "a signature layer names no blob"))?;
        let blob = format!("https://{host}/v2/{path}/blobs/{digest}");
        let payload = reaching
            .send(&Request {
                method: Method::Get,
                url: blob.clone(),
                headers: Vec::new(),
                body: None,
            })
            .await
            .map_err(|why| Unanswerable::about(image, &why.reason))?;
        if !payload.is_success() {
            return Err(Unanswerable::about(
                image,
                &format!("{blob} answered {}", payload.status),
            ));
        }
        offered.push(Offered {
            payload: payload.body,
            signature,
        });
    }
    Ok(offered)
}

#[cfg(test)]
mod tests;
