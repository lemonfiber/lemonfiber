//! Installing by name: resolved only through an index whose signature verified, at the
//! commit it reviewed, and recorded as reviewed with what signed it.

use async_trait::async_trait;
use lemonfiber_fixtures::http::Answer;

use super::fetching::{served, Serving};
use super::*;
use crate::plugin::catalogue::signing::Signing;
use crate::plugin::Source;
use crate::ports::http::{Request, Response, Unreachable};

/// Where the catalogue registers the plugin, and the commit of it that was reviewed.
const ORIGIN: &str = "https://github.com/lemonfiber/plugin-komga";
const REVIEWED: &str = "8fa05ba718f70624f2c122f8c0371d47e6c90d0e";

/// The digest of a manifest, as an index names one.
fn digest_of(manifest: &str) -> String {
    let taken = crate::secret::render(
        ring::digest::digest(&ring::digest::SHA256, manifest.as_bytes()).as_ref(),
    );
    format!("sha256:{taken}")
}

/// An index registering komga at `revision`, with the digest of `manifest`.
fn index(revision: &str, manifest: &str) -> String {
    format!(
        "{{\n  \"plugins\": [\n    {{\n      \"id\": \"komga\",\n      \"manifest\": \"{}\",\n      \
         \"origin\": \"{ORIGIN}\",\n      \"revision\": \"{revision}\"\n    }}\n  ],\n  \
         \"schema\": 1\n}}\n",
        digest_of(manifest)
    )
}

/// A release serving this index and this signature, where it serves one.
fn release(index: &str, signature: Option<String>) -> Arc<lemonfiber_fixtures::http::Fake> {
    Fake::by_path(vec![
        (
            "index.json.sig",
            signature.map_or_else(
                || Answer::reply(404, "Not Found"),
                |one| Answer::reply(200, one),
            ),
        ),
        ("index.json", Answer::reply(200, index)),
    ])
}

/// A context whose git source serves [`MANIFEST`], whose catalogue serves `http`, and
/// which carries `key`.
fn cataloguing(
    name: &str,
    http: Arc<dyn Http>,
    key: Option<crate::plugin::Key>,
) -> (Ctx, Arc<Serving>) {
    let serving = Arc::new(Serving::listing(""));
    let mut ctx = served(name, &serving).with_http(http);
    ctx.catalogue_key = key.map(Box::new);
    (ctx, serving)
}

/// Install `written`, as the operator would write it.
async fn by_name(ctx: &Ctx, written: &str) -> Result<Installs, Box<crate::error::Problem>> {
    plugins(
        ctx,
        &Asked::Install {
            source: Source::named(written),
        },
    )
    .await
}

/// A name resolves through a signed index to the origin and commit it registered; the
/// commit is fetched as it is named rather than asked for, and the record keeps it as
/// reviewed, with the key that signed the index.
#[tokio::test]
async fn a_name_is_installed_at_what_the_signed_index_reviewed() {
    let signing = Signing::new();
    assert!(signing.is_some(), "no key pair could be made");
    if let Some(signing) = signing {
        let listed = index(REVIEWED, MANIFEST);
        let (ctx, serving) = cataloguing(
            "catalogue-signed",
            release(&listed, Some(signing.signed(&listed))),
            signing.key(),
        );

        let installed =
            report(by_name(&ctx, "komga").await).and_then(|one| one.installed.first().cloned());

        assert_eq!(
            installed.as_ref().map(|one| (
                one.from.as_str(),
                one.revision.as_str(),
                one.declared.reviewed
            )),
            Some((ORIGIN, REVIEWED, true))
        );
        assert!(
            installed
                .as_ref()
                .is_some_and(|one| one.signed.starts_with("the catalogue's test key (sha256:")),
            "{installed:?}"
        );
        let asked = serving.asked();
        assert!(
            !asked
                .iter()
                .any(|one| one.first().map(String::as_str) == Some("ls-remote")),
            "a reviewed commit was asked for rather than taken as named: {asked:?}"
        );
        assert!(
            asked.iter().any(|one| one.contains(&ORIGIN.to_owned())),
            "{asked:?}"
        );
    }
}

/// An index the carried key did not sign, one with no signature, and one read by a
/// build carrying no key are refused alike, and no origin is fetched from.
#[tokio::test]
async fn an_index_that_does_not_verify_resolves_nothing() {
    let pairs = Signing::new().zip(Signing::new());
    assert!(pairs.is_some(), "no key pair could be made");
    if let Some((ours, theirs)) = pairs {
        let listed = index(REVIEWED, MANIFEST);
        for (name, signature, key) in [
            ("catalogue-theirs", Some(theirs.signed(&listed)), ours.key()),
            ("catalogue-unsigned", None, ours.key()),
            ("catalogue-keyless", Some(ours.signed(&listed)), None),
        ] {
            let (ctx, serving) = cataloguing(name, release(&listed, signature), key);

            assert_eq!(refusal(by_name(&ctx, "komga").await), "PLUGIN-20", "{name}");
            assert!(serving.asked().is_empty(), "{name}: {:?}", serving.asked());
            assert_eq!(counted(reading(&ctx).await), Some(0), "{name}");
        }
    }
}

/// A name the verified index does not hold is refused naming it, and a revision that is
/// not one whole commit is not one an index may pin.
#[tokio::test]
async fn a_name_the_index_does_not_pin_is_refused() {
    let signing = Signing::new();
    assert!(signing.is_some(), "no key pair could be made");
    if let Some(signing) = signing {
        let listed = index(REVIEWED, MANIFEST);
        let (ctx, _) = cataloguing(
            "catalogue-absent",
            release(&listed, Some(signing.signed(&listed))),
            signing.key(),
        );
        let absent = by_name(&ctx, "kuma").await.err();
        assert_eq!(
            absent
                .as_ref()
                .map(|problem| (problem.code.to_string(), problem.summary.clone())),
            Some((
                "PLUGIN-22".to_owned(),
                "The catalogue holds no plugin called kuma".to_owned()
            ))
        );

        let floating = index("main", MANIFEST);
        let (ctx, serving) = cataloguing(
            "catalogue-floating",
            release(&floating, Some(signing.signed(&floating))),
            signing.key(),
        );
        assert_eq!(refusal(by_name(&ctx, "komga").await), "PLUGIN-21");
        assert!(serving.asked().is_empty(), "{:?}", serving.asked());
    }
}

/// A signed index of a shape this build does not read is refused as unreadable, and one
/// whose signature cannot be fetched is refused as unreachable rather than unsigned.
#[tokio::test]
async fn an_unreadable_index_or_an_unfetchable_signature_resolves_nothing() {
    let signing = Signing::new();
    assert!(signing.is_some(), "no key pair could be made");
    if let Some(signing) = signing {
        let later = index(REVIEWED, MANIFEST).replace("\"schema\": 1", "\"schema\": 2");
        let (ctx, serving) = cataloguing(
            "catalogue-later",
            release(&later, Some(signing.signed(&later))),
            signing.key(),
        );
        assert_eq!(refusal(by_name(&ctx, "komga").await), "PLUGIN-21");
        assert!(serving.asked().is_empty(), "{:?}", serving.asked());

        let listed = index(REVIEWED, MANIFEST);
        let (ctx, serving) = cataloguing(
            "catalogue-unsigned-down",
            Fake::by_path(vec![
                ("index.json.sig", Answer::reply(503, "")),
                ("index.json", Answer::reply(200, listed)),
            ]),
            signing.key(),
        );
        assert_eq!(refusal(by_name(&ctx, "komga").await), "PLUGIN-19");
        assert!(serving.asked().is_empty(), "{:?}", serving.asked());
    }
}

/// A commit holding a manifest other than the one the catalogue reviewed is refused
/// before it is installed, and nothing is recorded.
#[tokio::test]
async fn a_manifest_other_than_the_reviewed_one_is_refused() {
    let signing = Signing::new();
    assert!(signing.is_some(), "no key pair could be made");
    if let Some(signing) = signing {
        let listed = index(REVIEWED, PROVING);
        let (ctx, _) = cataloguing(
            "catalogue-moved",
            release(&listed, Some(signing.signed(&listed))),
            signing.key(),
        );

        assert_eq!(refusal(by_name(&ctx, "komga").await), "PLUGIN-23");
        assert_eq!(counted(reading(&ctx).await), Some(0));
        assert!(super::super::fetching::checkout(&ctx, REVIEWED).is_some_and(|at| !at.exists()));
    }
}

/// Switched off, the catalogue is never asked; unreachable, an install by name is
/// refused while one from a directory goes ahead without asking it.
#[tokio::test]
async fn a_catalogue_switched_off_or_unreachable_stops_only_an_install_by_name() {
    let silent = Fake::silent();
    let (mut ctx, _) = cataloguing("catalogue-off", silent.clone(), None);
    ctx.settings.reaching = crate::config::Reaching::without(crate::config::REACH_CATALOGUE_KEY);
    assert_eq!(refusal(by_name(&ctx, "komga").await), "PLUGIN-18");
    assert!(silent.requests().is_empty(), "{:?}", silent.requests());

    let (ctx, _) = cataloguing("catalogue-down", silent.clone(), None);
    assert_eq!(refusal(by_name(&ctx, "komga").await), "PLUGIN-19");

    let asked_before = of_the_catalogue(&silent.requests());
    let installed = counted(installing(&ctx, &source("catalogue-down", MANIFEST)).await);
    assert_eq!(installed, Some(1));
    assert_eq!(
        of_the_catalogue(&silent.requests()),
        asked_before,
        "a directory install asked the catalogue"
    );
}

/// How many of these requests went to the catalogue's release.
fn of_the_catalogue(requests: &[Request]) -> usize {
    requests
        .iter()
        .filter(|one| one.url.contains("lemonfiber-plugins"))
        .count()
}

/// A catalogue that has published no release has no index to resolve through.
#[tokio::test]
async fn a_catalogue_with_no_release_is_refused_as_unreachable() {
    let (ctx, _) = cataloguing(
        "catalogue-empty",
        Fake::always(Answer::reply(404, "Not Found")),
        None,
    );

    let refused = by_name(&ctx, "komga").await.err();
    assert_eq!(
        refused
            .as_ref()
            .map(|problem| (problem.code.to_string(), problem.detail.clone())),
        Some((
            "PLUGIN-19".to_owned(),
            Some("the catalogue has published no release, so there is no index to read".to_owned())
        ))
    );
}

/// A release address that hands each file on to the host that serves it.
struct HandingOn {
    /// Where the index's address hands it on to.
    index_at: &'static str,
    index: String,
    signature: String,
    asked: std::sync::Mutex<Vec<String>>,
}

impl HandingOn {
    fn to(index_at: &'static str, index: String, signature: String) -> Arc<Self> {
        Arc::new(Self {
            index_at,
            index,
            signature,
            asked: std::sync::Mutex::new(Vec::new()),
        })
    }

    fn asked(&self) -> Vec<String> {
        self.asked
            .lock()
            .map(|asked| asked.clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl Http for HandingOn {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        if let Ok(mut asked) = self.asked.lock() {
            asked.push(request.url.clone());
        }
        let handed = |to: &str| Response {
            status: 302,
            headers: vec![("location".to_owned(), to.to_owned())],
            body: String::new(),
        };
        let served = |body: &str| Response {
            status: 200,
            headers: Vec::new(),
            body: body.to_owned(),
        };
        Ok(match request.url.as_str() {
            crate::plugin::catalogue::INDEX => handed(self.index_at),
            crate::plugin::catalogue::SIGNATURE => handed("https://assets.example/sig"),
            "https://assets.example/index" => served(&self.index),
            "https://assets.example/sig" => served(&self.signature),
            _ => handed(request.url.as_str()),
        })
    }
}

/// The address a release file is handed on to is followed where it is encrypted, and
/// what it serves is verified as though it had been served directly.
#[tokio::test]
async fn a_release_file_handed_on_is_followed_and_verified() {
    let signing = Signing::new();
    assert!(signing.is_some(), "no key pair could be made");
    if let Some(signing) = signing {
        let listed = index(REVIEWED, MANIFEST);
        let handing = HandingOn::to(
            "https://assets.example/index",
            listed.clone(),
            signing.signed(&listed),
        );
        let (ctx, _) = cataloguing("catalogue-handed-on", handing.clone(), signing.key());

        assert_eq!(counted(by_name(&ctx, "komga").await), Some(1));
        assert_eq!(
            handing
                .asked()
                .into_iter()
                .filter(|url| url.contains("lemonfiber-plugins") || url.contains("assets.example"))
                .collect::<Vec<_>>(),
            vec![
                crate::plugin::catalogue::INDEX.to_owned(),
                "https://assets.example/index".to_owned(),
                crate::plugin::catalogue::SIGNATURE.to_owned(),
                "https://assets.example/sig".to_owned(),
            ]
        );
    }
}

/// A hop to an unencrypted address is not followed, and one that never lands is given
/// up after a few.
#[tokio::test]
async fn a_hop_that_is_unencrypted_or_endless_is_refused() {
    for (name, index_at, asked) in [
        ("catalogue-plain", "http://assets.example/index", 1),
        ("catalogue-loop", "https://assets.example/loop", 4),
    ] {
        let handing = HandingOn::to(index_at, String::new(), String::new());
        let (ctx, _) = cataloguing(name, handing.clone(), None);

        assert_eq!(refusal(by_name(&ctx, "komga").await), "PLUGIN-19", "{name}");
        assert_eq!(handing.asked().len(), asked, "{name}");
    }
}
