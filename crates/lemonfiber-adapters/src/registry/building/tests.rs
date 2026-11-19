use lemonfiber_fixtures::http::Fake;
use lemonfiber_ports::http::Response;
use lemonfiber_ports::registry::{Built, Image, Registry, Statement};

use crate::registry::served::tests::{answered, named, serving};
use crate::registry::Oci;

/// A platform's image, the layers its attestation lists, and their bodies.
struct Platform {
    image: String,
    layers: Vec<(&'static str, String)>,
}

/// What a registry serves for an index listing each of `platforms` and, beside each one
/// with layers, its attestation: the index first.
fn published(platforms: &[Platform]) -> (String, Vec<(String, Response)>) {
    let mut routes = Vec::new();
    let mut listed = Vec::new();
    for platform in platforms {
        listed.push(format!(
            r#"{{"digest":"{}","platform":{{"os":"linux","architecture":"amd64"}}}}"#,
            platform.image
        ));
        if platform.layers.is_empty() {
            continue;
        }
        let layers: Vec<String> = platform
            .layers
            .iter()
            .map(|(kind, body)| {
                routes.push((named(body), answered(200, &[], body)));
                format!(
                    r#"{{"digest":"{}","annotations":{{"in-toto.io/predicate-type":"{kind}"}}}}"#,
                    named(body)
                )
            })
            .collect();
        let attestation = format!(r#"{{"schemaVersion":2,"layers":[{}]}}"#, layers.join(","));
        listed.push(format!(
            r#"{{"digest":"{}","annotations":{{"vnd.docker.reference.digest":"{}","vnd.docker.reference.type":"attestation-manifest"}},"platform":{{"os":"unknown","architecture":"unknown"}}}}"#,
            named(&attestation),
            platform.image
        ));
        routes.push((named(&attestation), answered(200, &[], &attestation)));
    }
    let index = format!(
        r#"{{"schemaVersion":2,"manifests":[{}]}}"#,
        listed.join(",")
    );
    routes.insert(0, (named(&index), answered(200, &[], &index)));
    (named(&index), routes)
}

const SLSA: &str = "https://slsa.dev/provenance/v1";
const SPDX: &str = "https://spdx.dev/Document";

fn caddy(digest: &str) -> Image {
    Image::new("ghcr.io/lemonfiber/caddy", digest)
}

#[tokio::test]
async fn each_build_provenance_statement_is_read_beside_the_image_it_attests() {
    let amd64 = named("amd64");
    let (digest, routes) = published(&[
        Platform {
            image: amd64.clone(),
            layers: vec![
                (SPDX, "the sbom".to_owned()),
                (SLSA, "the provenance".to_owned()),
            ],
        },
        Platform {
            image: named("arm64"),
            layers: Vec::new(),
        },
    ]);
    let built = Oci::new(serving(routes)).built(&caddy(&digest)).await;
    assert_eq!(
        built,
        Ok(Built {
            images: vec![amd64.clone(), named("arm64")],
            statements: vec![Statement {
                attests: amd64,
                body: "the provenance".to_owned(),
            }],
        })
    );
}

#[tokio::test]
async fn an_image_that_is_no_index_is_its_own_one_image_with_no_statement() {
    let manifest = r#"{"schemaVersion":2,"config":{"digest":"sha256:c"},"layers":[]}"#;
    let routes = vec![(named(manifest), answered(200, &[], manifest))];
    let built = Oci::new(serving(routes))
        .built(&caddy(&named(manifest)))
        .await;
    assert_eq!(
        built,
        Ok(Built {
            images: vec![named(manifest)],
            statements: Vec::new(),
        })
    );
}

#[tokio::test]
async fn an_index_entry_naming_no_attested_image_is_passed_over() {
    let index = format!(
        r#"{{"manifests":[{{"digest":"{}","annotations":{{"vnd.docker.reference.type":"attestation-manifest"}},"platform":{{"os":"unknown"}}}}]}}"#,
        named("unread")
    );
    let routes = vec![(named(&index), answered(200, &[], &index))];
    let asked = serving(routes);
    let built = Oci::new(asked.clone()).built(&caddy(&named(&index))).await;
    assert_eq!(built, Ok(Built::default()));
    assert_eq!(asked.requests().len(), 1);
}

#[tokio::test]
async fn build_provenance_that_cannot_be_read_is_unanswerable_about_the_image() {
    let (digest, routes) = published(&[Platform {
        image: named("amd64"),
        layers: vec![(SLSA, "the provenance".to_owned())],
    }]);
    let mut tampered = routes.clone();
    if let Some((_, statement)) = tampered.last_mut() {
        statement.body = r#"{"schemaVersion":2}"#.to_owned();
    }
    let unnamed = r#"{"manifests":[{"platform":{"os":"linux"}}]}"#;
    let blobless = format!(
        r#"{{"schemaVersion":2,"layers":[{{"annotations":{{"in-toto.io/predicate-type":"{SLSA}"}}}}]}}"#
    );
    let blobless_index = format!(
        r#"{{"manifests":[{{"digest":"{}","annotations":{{"vnd.docker.reference.digest":"sha256:a","vnd.docker.reference.type":"attestation-manifest"}},"platform":{{"os":"unknown"}}}}]}}"#,
        named(&blobless)
    );
    let cases: Vec<(String, Vec<(String, Response)>)> = vec![
        (digest.clone(), tampered),
        (digest.clone(), routes.into_iter().take(1).collect()),
        (
            named(unnamed),
            vec![(named(unnamed), answered(200, &[], unnamed))],
        ),
        (
            named(&blobless_index),
            vec![
                (named(&blobless_index), answered(200, &[], &blobless_index)),
                (named(&blobless), answered(200, &[], &blobless)),
            ],
        ),
        (digest, Vec::new()),
    ];
    for (asked, routes) in cases {
        let built = Oci::new(serving(routes)).built(&caddy(&asked)).await;
        assert!(
            built.as_ref().is_err_and(|why| why.asked == asked),
            "{built:?}"
        );
    }
    assert!(Oci::new(Fake::silent())
        .built(&caddy(&named("any")))
        .await
        .is_err());
}
