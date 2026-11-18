use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::registered::{ELSEWHERE, REVISION};
use lemonfiber_ports::http::Http;
use lemonfiber_ports::registry::{Image, Registry, Tagged};

use super::{addressed, beside, challenged, escaped, Oci};

/// The fake, as the port takes a transport.
fn reaching(fake: Arc<Fake>) -> Arc<dyn Http> {
    fake
}

const DIGEST: &str = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945";

fn komga() -> Image {
    Image::new("docker.io/gotson/komga", DIGEST)
}

/// A signature manifest as the tooling that writes one produces it.
const MANIFEST: &str = r#"{"schemaVersion":2,"layers":[
    {"digest":"sha256:aaa","annotations":{"dev.cosignproject.cosign/signature":"AQID"}}
]}"#;

#[tokio::test]
async fn a_repository_with_nothing_signed_in_it_has_answered() {
    let asked = reaching(Fake::always(Answer::reply(404, "{}")));
    let found = Oci::new(asked).signatures(&komga()).await;
    assert_eq!(found, Ok(Vec::new()), "404 is an answer, not an outage");
}

#[tokio::test]
async fn a_registry_that_answered_nothing_is_a_question_nobody_put() {
    let found = Oci::new(reaching(Fake::silent()))
        .signatures(&komga())
        .await;
    assert!(found.is_err(), "silence was read as nothing being signed");
}

/// A refusal is not an answer about the image either.
#[tokio::test]
async fn a_repository_this_build_may_not_look_in_is_unanswerable() {
    for status in [401, 403, 429, 500] {
        let asked = reaching(Fake::always(Answer::reply(status, "{}")));
        assert!(
            Oci::new(asked).signatures(&komga()).await.is_err(),
            "{status} was read as an answer about the image"
        );
    }
}

#[tokio::test]
async fn a_signature_is_read_out_of_its_layer_and_its_blob() {
    let asked = reaching(Fake::by_path(vec![
        ("/manifests/", Answer::reply(200, MANIFEST)),
        ("/blobs/", Answer::reply(200, "the payload")),
    ]));
    let found = Oci::new(asked).signatures(&komga()).await;
    let one = found.as_ref().ok().and_then(|found| found.first());
    assert_eq!(one.map(|one| one.payload.as_str()), Some("the payload"));
    assert_eq!(one.map(|one| one.signature.clone()), Some(vec![1, 2, 3]));
}

#[tokio::test]
async fn an_answer_this_build_cannot_read_is_unanswerable_rather_than_empty() {
    for unreadable in ["not json", r#"{"schemaVersion":2}"#] {
        let asked = reaching(Fake::always(Answer::reply(200, unreadable)));
        assert!(
            Oci::new(asked).signatures(&komga()).await.is_err(),
            "{unreadable} was read as a repository with nothing signed in it"
        );
    }
}

/// A layer carrying no signature is skipped, not turned into an empty one.
#[tokio::test]
async fn a_layer_that_is_not_a_signature_contributes_nothing() {
    let asked = reaching(Fake::always(Answer::reply(
        200,
        r#"{"schemaVersion":2,"layers":[{"digest":"sha256:bbb","annotations":{}}]}"#,
    )));
    assert_eq!(Oci::new(asked).signatures(&komga()).await, Ok(Vec::new()));
}

/// A layer that says it carries a signature and does not is unanswerable.
///
/// Each of these is the registry answering in a shape this build cannot read,
/// which says nothing about the image — so none of them may come back as a
/// repository with nothing signed in it.
#[tokio::test]
async fn a_layer_this_build_cannot_read_is_a_question_nobody_put() {
    let cases = [
        (
            "a signature that is not base64",
            r#"{"schemaVersion":2,"layers":[{"digest":"sha256:aaa","annotations":{"dev.cosignproject.cosign/signature":"!!!!"}}]}"#,
        ),
        (
            "a signature layer naming no blob",
            r#"{"schemaVersion":2,"layers":[{"annotations":{"dev.cosignproject.cosign/signature":"AQID"}}]}"#,
        ),
    ];
    assert_eq!(cases.len(), 2, "no layer was put in front of this");
    for (what, manifest) in cases {
        let asked = reaching(Fake::always(Answer::reply(200, manifest)));
        assert!(
            Oci::new(asked).signatures(&komga()).await.is_err(),
            "{what} was read as an answer about the image"
        );
    }
}

/// The blob is the thing that was signed, and a registry that will not serve it
/// has not said anything about the image either.
///
/// Two ways it will not: a refusal it gave, and a connection that never
/// answered. The manifest is served in both, so what is being read is the
/// second half of the errand failing rather than the first.
#[tokio::test]
async fn a_payload_the_registry_would_not_serve_is_a_question_nobody_put() {
    let refused = reaching(Fake::by_path(vec![
        ("/manifests/", Answer::reply(200, MANIFEST)),
        ("/blobs/", Answer::reply(500, "")),
    ]));
    let why = Oci::new(refused)
        .signatures(&komga())
        .await
        .err()
        .map(|one| one.reason)
        .unwrap_or_default();
    assert!(why.contains("answered 500"), "{why}");

    let silent = reaching(Fake::by_path(vec![
        ("/manifests/", Answer::reply(200, MANIFEST)),
        ("/blobs/", Answer::Silent),
    ]));
    assert!(
        Oci::new(silent).signatures(&komga()).await.is_err(),
        "a blob nobody answered for became a signature"
    );
}

#[test]
fn a_signature_sits_beside_its_image_under_a_tag_derived_from_the_digest() {
    assert_eq!(beside(DIGEST), format!("sha256-{}.sig", &DIGEST[7..]));
}

/// Where a repository name actually points, including the two facts about the
/// registry most operators are using without having named it.
#[test]
fn a_repository_name_resolves_to_the_host_that_serves_it() {
    assert_eq!(
        addressed("ghcr.io/lemonfiber/x"),
        ("ghcr.io".to_owned(), "lemonfiber/x".to_owned())
    );
    assert_eq!(
        addressed("docker.io/gotson/komga"),
        ("registry-1.docker.io".to_owned(), "gotson/komga".to_owned())
    );
    assert_eq!(
        addressed("postgres"),
        (
            "registry-1.docker.io".to_owned(),
            "library/postgres".to_owned()
        )
    );
    assert_eq!(
        addressed("localhost:5000/x"),
        ("localhost:5000".to_owned(), "x".to_owned())
    );
    assert_eq!(
        addressed("localhost/x"),
        ("localhost".to_owned(), "x".to_owned())
    );
}

fn lemonfiber() -> Tagged {
    Tagged::new("ghcr.io/lemonfiber/lemonfiber", "0.18.0")
}

fn headed(status: u16, headers: &[(&str, &str)], body: &str) -> Answer {
    Answer::Headed {
        status,
        headers: headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
        body: body.to_owned(),
    }
}

const GHCR_CHALLENGE: &str = r#"Bearer realm="https://ghcr.io/token",service="ghcr.io",scope="repository:lemonfiber/lemonfiber:pull""#;

fn bearer(request: &lemonfiber_ports::http::Request) -> Option<&str> {
    request
        .headers
        .iter()
        .find(|(name, _)| name == "Authorization")
        .map(|(_, value)| value.as_str())
}

#[tokio::test]
async fn a_tag_is_pinned_at_the_digest_the_registry_names_for_it() {
    let asked = Fake::always(headed(200, &[("docker-content-digest", DIGEST)], "{}"));
    let pinned = Oci::new(reaching(Arc::clone(&asked)))
        .digest(&lemonfiber())
        .await;
    assert_eq!(pinned, Ok(Some(DIGEST.to_owned())));
    let request = asked
        .request()
        .unwrap_or_else(|| unreachable!("nothing was asked"));
    assert!(
        request
            .url
            .ends_with("/v2/lemonfiber/lemonfiber/manifests/0.18.0"),
        "{}",
        request.url
    );
    assert!(
        request.headers.iter().any(|(name, value)| name == "Accept"
            && value.starts_with("application/vnd.oci.image.index.v1+json")),
        "an image index was not asked for first"
    );
}

#[tokio::test]
async fn a_tag_nothing_is_published_under_has_answered() {
    let asked = reaching(Fake::always(Answer::reply(404, "{}")));
    assert_eq!(Oci::new(asked).digest(&lemonfiber()).await, Ok(None));
}

#[tokio::test]
async fn a_tag_answered_without_a_digest_in_its_shape_is_unanswerable() {
    for headers in [
        vec![],
        vec![("Docker-Content-Digest", "sha256:abc")],
        vec![("Docker-Content-Digest", "md5:00")],
    ] {
        let asked = reaching(Fake::always(headed(200, &headers, "{}")));
        assert!(
            Oci::new(asked).digest(&lemonfiber()).await.is_err(),
            "{headers:?}"
        );
    }
    let refused = reaching(Fake::always(Answer::reply(500, "{}")));
    assert!(Oci::new(refused).digest(&lemonfiber()).await.is_err());
}

#[tokio::test]
async fn a_refusal_with_a_challenge_is_asked_again_with_the_token_it_names() {
    let asked = Fake::by_path_in_turn(vec![
        (
            "/token",
            vec![Answer::reply(200, r#"{"token":"anonymous"}"#)],
        ),
        (
            "/manifests/",
            vec![
                headed(401, &[("WWW-Authenticate", GHCR_CHALLENGE)], ""),
                headed(200, &[("Docker-Content-Digest", DIGEST)], "{}"),
            ],
        ),
    ]);
    let pinned = Oci::new(reaching(Arc::clone(&asked)))
        .digest(&lemonfiber())
        .await;
    assert_eq!(pinned, Ok(Some(DIGEST.to_owned())));
    let requests = asked.requests();
    assert_eq!(
        requests
            .iter()
            .map(|request| request.url.as_str())
            .collect::<Vec<_>>(),
        vec![
            "https://ghcr.io/v2/lemonfiber/lemonfiber/manifests/0.18.0",
            "https://ghcr.io/token?service=ghcr.io&scope=repository:lemonfiber/lemonfiber:pull",
            "https://ghcr.io/v2/lemonfiber/lemonfiber/manifests/0.18.0",
        ]
    );
    assert_eq!(requests.first().and_then(bearer), None);
    assert_eq!(
        requests.get(1).and_then(bearer),
        None,
        "the token was asked for with a token"
    );
    assert_eq!(requests.get(2).and_then(bearer), Some("Bearer anonymous"));
}

#[tokio::test]
async fn a_signature_read_carries_the_token_to_its_blob() {
    let ghcr = Image::new("ghcr.io/lemonfiber/lemonfiber", DIGEST);
    let asked = Fake::by_path_in_turn(vec![
        (
            "/token",
            vec![Answer::reply(200, r#"{"access_token":"anonymous"}"#)],
        ),
        (
            "/manifests/",
            vec![
                headed(401, &[("WWW-Authenticate", GHCR_CHALLENGE)], ""),
                Answer::reply(200, MANIFEST),
            ],
        ),
        ("/blobs/", vec![Answer::reply(200, "payload")]),
    ]);
    let found = Oci::new(reaching(Arc::clone(&asked)))
        .signatures(&ghcr)
        .await;
    assert_eq!(found.map(|offered| offered.len()), Ok(1));
    let blob = asked
        .requests()
        .into_iter()
        .find(|request| request.url.contains("/blobs/"))
        .unwrap_or_else(|| unreachable!("the blob was not asked for"));
    assert_eq!(bearer(&blob), Some("Bearer anonymous"));
}

#[tokio::test]
async fn a_challenge_this_build_does_not_follow_leaves_the_refusal_standing() {
    for challenge in [
        r#"Bearer realm="https://elsewhere.example/token",service="ghcr.io""#,
        r#"Bearer realm="http://ghcr.io/token",service="ghcr.io""#,
        r#"Basic realm="ghcr.io""#,
    ] {
        let asked = Fake::always(headed(401, &[("WWW-Authenticate", challenge)], ""));
        let pinned = Oci::new(reaching(Arc::clone(&asked)))
            .digest(&lemonfiber())
            .await;
        assert!(pinned.is_err(), "{challenge}");
        assert_eq!(asked.requests().len(), 1, "{challenge} was followed");
    }
}

#[tokio::test]
async fn a_token_service_that_hands_out_nothing_is_unanswerable() {
    for token in [
        Answer::reply(500, "{}"),
        Answer::reply(200, "not json"),
        Answer::reply(200, "{}"),
    ] {
        let asked = Fake::by_path_in_turn(vec![
            ("/token", vec![token.clone()]),
            (
                "/manifests/",
                vec![headed(401, &[("WWW-Authenticate", GHCR_CHALLENGE)], "")],
            ),
        ]);
        assert!(
            Oci::new(reaching(asked))
                .digest(&lemonfiber())
                .await
                .is_err(),
            "{token:?}"
        );
    }
}

#[test]
fn docker_hub_hands_its_tokens_out_from_its_own_token_host() {
    assert_eq!(
        challenged(
            "registry-1.docker.io",
            r#"Bearer realm="https://auth.docker.io/token",service="registry.docker.io",scope="repository:library/postgres:pull""#
        ),
        Some("https://auth.docker.io/token?service=registry.docker.io&scope=repository:library/postgres:pull".to_owned())
    );
    assert_eq!(
        challenged("ghcr.io", r#"Bearer realm="https://auth.docker.io/token""#),
        None
    );
    assert_eq!(
        challenged("ghcr.io", r#"Bearer realm="https://ghcr.io/token""#),
        Some("https://ghcr.io/token".to_owned())
    );
}

#[test]
fn a_query_value_carries_only_what_it_may_as_written() {
    assert_eq!(escaped("repository:a/b:pull"), "repository:a/b:pull");
    assert_eq!(escaped("a b&c=d"), "a%20b%26c%3Dd");
}

/// The image the pass service runs, by the digest its tag was pinned at.
fn pinned_lemonfiber() -> Image {
    Image::new("ghcr.io/lemonfiber/lemonfiber", DIGEST)
}

/// An image index listing an image for each of `platforms` and, beside them, what a
/// builder lists for no platform.
fn index(platforms: &[&str]) -> String {
    let listed: Vec<String> = platforms
        .iter()
        .map(|arch| {
            format!(
                r#"{{"digest":"sha256:{arch}","platform":{{"os":"linux","architecture":"{arch}"}}}}"#
            )
        })
        .chain(std::iter::once(
            r#"{"digest":"sha256:attestation","platform":{"os":"unknown","architecture":"unknown"}}"#
                .to_owned(),
        ))
        .collect();
    format!(
        r#"{{"schemaVersion":2,"manifests":[{}]}}"#,
        listed.join(",")
    )
}

/// An image manifest naming the configuration `config`.
fn image(config: &str) -> Answer {
    Answer::reply(
        200,
        format!(r#"{{"schemaVersion":2,"config":{{"digest":"sha256:{config}"}},"layers":[]}}"#),
    )
}

/// An image configuration labelled with `revision`, or with no revision.
fn configured(revision: Option<&str>) -> Answer {
    let labels = revision.map_or_else(String::new, |revision| {
        format!(r#""org.opencontainers.image.revision":"{revision}""#)
    });
    Answer::reply(200, format!(r#"{{"config":{{"Labels":{{{labels}}}}}}}"#))
}

#[tokio::test]
async fn an_index_whose_platforms_are_built_from_one_commit_names_it() {
    let asked = Fake::by_path(vec![
        (DIGEST, Answer::reply(200, index(&["amd64", "arm64"]))),
        ("/manifests/sha256:amd64", image("amd64-config")),
        ("/manifests/sha256:arm64", image("arm64-config")),
        ("/blobs/sha256:amd64-config", configured(Some(REVISION))),
        ("/blobs/sha256:arm64-config", configured(Some(REVISION))),
    ]);
    let revision = Oci::new(reaching(Arc::clone(&asked)))
        .revision(&pinned_lemonfiber())
        .await;
    assert_eq!(revision, Ok(Some(REVISION.to_owned())));
    let urls: Vec<String> = asked
        .requests()
        .into_iter()
        .map(|request| request.url)
        .collect();
    assert_eq!(
        urls,
        vec![
            format!("https://ghcr.io/v2/lemonfiber/lemonfiber/manifests/{DIGEST}"),
            "https://ghcr.io/v2/lemonfiber/lemonfiber/manifests/sha256:amd64".to_owned(),
            "https://ghcr.io/v2/lemonfiber/lemonfiber/blobs/sha256:amd64-config".to_owned(),
            "https://ghcr.io/v2/lemonfiber/lemonfiber/manifests/sha256:arm64".to_owned(),
            "https://ghcr.io/v2/lemonfiber/lemonfiber/blobs/sha256:arm64-config".to_owned(),
        ],
        "what an index lists for no platform was read"
    );
}

#[tokio::test]
async fn an_index_whose_platforms_do_not_name_one_commit_names_none() {
    for (amd64, arm64) in [
        (Some(REVISION), Some(ELSEWHERE)),
        (Some(REVISION), None),
        (None, Some(REVISION)),
    ] {
        let asked = reaching(Fake::by_path(vec![
            (DIGEST, Answer::reply(200, index(&["amd64", "arm64"]))),
            ("/manifests/sha256:amd64", image("amd64-config")),
            ("/manifests/sha256:arm64", image("arm64-config")),
            ("/blobs/sha256:amd64-config", configured(amd64)),
            ("/blobs/sha256:arm64-config", configured(arm64)),
        ]));
        assert_eq!(
            Oci::new(asked).revision(&pinned_lemonfiber()).await,
            Ok(None),
            "{amd64:?} {arm64:?}"
        );
    }
    let empty = reaching(Fake::always(Answer::reply(200, index(&[]))));
    assert_eq!(
        Oci::new(empty).revision(&pinned_lemonfiber()).await,
        Ok(None)
    );
}

#[tokio::test]
async fn an_image_names_the_commit_its_configuration_is_labelled_with_or_none() {
    for revision in [Some(REVISION), None] {
        let asked = reaching(Fake::by_path(vec![
            (DIGEST, image("config")),
            ("/blobs/sha256:config", configured(revision)),
        ]));
        assert_eq!(
            Oci::new(asked).revision(&pinned_lemonfiber()).await,
            Ok(revision.map(str::to_owned))
        );
    }
}

#[tokio::test]
async fn an_image_whose_commit_cannot_be_read_is_unanswerable() {
    let unreadable = [
        vec![(DIGEST, Answer::reply(500, "{}"))],
        vec![(DIGEST, Answer::reply(200, "not json"))],
        vec![(DIGEST, Answer::reply(200, r#"{"schemaVersion":2}"#))],
        vec![(
            DIGEST,
            Answer::reply(200, r#"{"manifests":[{"platform":{"os":"linux"}}]}"#),
        )],
        vec![
            (DIGEST, Answer::reply(200, index(&["amd64"]))),
            ("/manifests/sha256:amd64", Answer::reply(404, "{}")),
        ],
        vec![
            (DIGEST, Answer::reply(200, index(&["amd64"]))),
            ("/manifests/sha256:amd64", Answer::reply(200, "{}")),
        ],
        vec![
            (DIGEST, image("config")),
            ("/blobs/sha256:config", Answer::reply(404, "{}")),
        ],
    ];
    for routes in unreadable {
        let asked = reaching(Fake::by_path(routes));
        let revision = Oci::new(asked).revision(&pinned_lemonfiber()).await;
        assert!(
            revision.as_ref().is_err_and(|why| why.asked == DIGEST),
            "{revision:?}"
        );
    }
    let silent = Oci::new(reaching(Fake::silent()))
        .revision(&pinned_lemonfiber())
        .await;
    assert!(silent.is_err());
}
