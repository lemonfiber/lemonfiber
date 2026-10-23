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
//! A registry that serves anonymous readers only with a token says so by answering 401
//! with a bearer challenge, and the token it names is fetched and the question put once
//! more. The token is anonymous: nothing about this machine travels to get it.
//!
//! Everything else that goes wrong is [`Unanswerable`]. A token this build does not
//! hold, a rate limit, an answer in a shape it cannot read — none of them says
//! anything about the image, and each is reported as the question not having been
//! put.

use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine as _;
use lemonfiber_ports::http::{Http, Method, Request, Response};
use lemonfiber_ports::registry::{is_digest, Image, Offered, Registry, Tagged, Unanswerable};

/// Where the signature's own bytes sit on a layer.
const SIGNATURE: &str = "dev.cosignproject.cosign/signature";

/// What a registry is told this build will accept.
const MANIFESTS: &str = "application/vnd.oci.image.manifest.v1+json, \
                         application/vnd.docker.distribution.manifest.v2+json";

/// What a registry is told this build will accept for a tag: an image index first, so
/// the digest it answers with is the one every platform's image is reached from.
const INDEXES: &str = "application/vnd.oci.image.index.v1+json, \
                       application/vnd.docker.distribution.manifest.list.v2+json, \
                       application/vnd.oci.image.manifest.v1+json, \
                       application/vnd.docker.distribution.manifest.v2+json";

/// What a registry is told this build will accept for a blob.
const ANYTHING: &str = "*/*";

/// The header a registry names the digest of what it served in.
const CONTENT_DIGEST: &str = "Docker-Content-Digest";

/// The header a registry challenges a reader without a token in.
const CHALLENGE: &str = "WWW-Authenticate";

/// The host Docker Hub hands its tokens out from, which is not the host it serves from.
const DOCKER_HUB_TOKENS: &str = "auth.docker.io";

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

    async fn digest(&self, tagged: &Tagged) -> Result<Option<String>, Unanswerable> {
        pinning(self.reaching.as_ref(), tagged).await
    }
}

/// The digest a registry publishes one tag at, asked over one transport.
async fn pinning(reaching: &dyn Http, tagged: &Tagged) -> Result<Option<String>, Unanswerable> {
    let (host, path) = addressed(&tagged.repository);
    let manifest = format!("https://{host}/v2/{path}/manifests/{}", tagged.tag);
    let answer = authorised(reaching, &host, &manifest, INDEXES, &mut None)
        .await
        .map_err(|why| Unanswerable::about_tag(tagged, &why))?;
    if answer.status == 404 {
        return Ok(None);
    }
    if !answer.is_success() {
        return Err(Unanswerable::about_tag(
            tagged,
            &format!("{manifest} answered {}", answer.status),
        ));
    }
    answer
        .header(CONTENT_DIGEST)
        .filter(|digest| is_digest(digest))
        .map(|digest| Some(digest.to_owned()))
        .ok_or_else(|| Unanswerable::about_tag(tagged, "it named no sha256 digest for the tag"))
}

/// What `url` on the registry at `host` answers, asked with the anonymous token `held`
/// keeps where one was handed out already, and asked once more with the token a
/// challenge names where the registry refuses a reader without a fresh one.
async fn authorised(
    reaching: &dyn Http,
    host: &str,
    url: &str,
    accept: &str,
    held: &mut Option<String>,
) -> Result<Response, String> {
    let asked = |token: Option<&str>| Request {
        method: Method::Get,
        url: url.to_owned(),
        headers: std::iter::once(("Accept".to_owned(), accept.to_owned()))
            .chain(token.map(|token| ("Authorization".to_owned(), format!("Bearer {token}"))))
            .collect(),
        body: None,
        pinned: None,
    };
    let answer = reaching
        .send(&asked(held.as_deref()))
        .await
        .map_err(|why| why.reason)?;
    if answer.status != 401 {
        return Ok(answer);
    }
    let Some(handed) = answer
        .header(CHALLENGE)
        .and_then(|challenge| challenged(host, challenge))
    else {
        return Ok(answer);
    };
    let token = held.insert(token(reaching, &handed).await?);
    reaching
        .send(&asked(Some(token)))
        .await
        .map_err(|why| why.reason)
}

/// Where a bearer challenge from the registry at `host` says its token is handed out:
/// its realm with the service and scope it names, where that realm is served over TLS
/// from the registry's own host, or from Docker Hub's token host for Docker Hub.
fn challenged(host: &str, challenge: &str) -> Option<String> {
    let named = challenge.strip_prefix("Bearer ")?;
    let field = |name: &str| {
        named.split(',').find_map(|pair| {
            let (key, value) = pair.trim().split_once('=')?;
            (key == name).then(|| value.trim_matches('"').to_owned())
        })
    };
    let realm = field("realm")?;
    let realm_host = realm.strip_prefix("https://")?.split('/').next()?;
    let trusted =
        realm_host == host || (host == "registry-1.docker.io" && realm_host == DOCKER_HUB_TOKENS);
    if !trusted {
        return None;
    }
    let query: Vec<String> = ["service", "scope"]
        .into_iter()
        .filter_map(|name| field(name).map(|value| format!("{name}={}", escaped(&value))))
        .collect();
    Some(if query.is_empty() {
        realm
    } else {
        format!("{realm}?{}", query.join("&"))
    })
}

/// `value`, with every byte a query value may not carry as written percent-encoded.
fn escaped(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-._~:/".contains(&byte) {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

/// The anonymous token `handed` gives out.
async fn token(reaching: &dyn Http, handed: &str) -> Result<String, String> {
    let answer = reaching
        .send(&Request {
            method: Method::Get,
            url: handed.to_owned(),
            headers: Vec::new(),
            body: None,
            pinned: None,
        })
        .await
        .map_err(|why| why.reason)?;
    if !answer.is_success() {
        return Err(format!("its token service answered {}", answer.status));
    }
    let read: serde_json::Value = serde_json::from_str(&answer.body)
        .map_err(|why| format!("its token service answered no JSON: {why}"))?;
    ["token", "access_token"]
        .into_iter()
        .find_map(|name| read.get(name).and_then(serde_json::Value::as_str))
        .map(str::to_owned)
        .ok_or_else(|| "its token service handed out no token".to_owned())
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
    let mut held = None;
    let answer = authorised(reaching, &host, &manifest, MANIFESTS, &mut held)
        .await
        .map_err(|why| Unanswerable::about(image, &why))?;

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
        let payload = authorised(reaching, &host, &blob, ANYTHING, &mut held)
            .await
            .map_err(|why| Unanswerable::about(image, &why))?;
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
