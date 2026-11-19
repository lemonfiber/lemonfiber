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
/// with layers, its attestation: the index first, each descriptor declaring its size.
fn published(platforms: &[Platform]) -> (String, Vec<(String, Response)>) {
    published_as(platforms, |_, size| size)
}

/// As [`published`], each descriptor declaring the size `declared` makes of whether it
/// names an attestation and the size of what it names.
fn published_as(
    platforms: &[Platform],
    declared: impl Fn(bool, usize) -> usize,
) -> (String, Vec<(String, Response)>) {
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
                    r#"{{"digest":"{}","size":{},"annotations":{{"in-toto.io/predicate-type":"{kind}"}}}}"#,
                    named(body),
                    declared(false, body.len())
                )
            })
            .collect();
        let attestation = format!(r#"{{"schemaVersion":2,"layers":[{}]}}"#, layers.join(","));
        listed.push(format!(
            r#"{{"digest":"{}","size":{},"annotations":{{"vnd.docker.reference.digest":"{}","vnd.docker.reference.type":"attestation-manifest"}},"platform":{{"os":"unknown","architecture":"unknown"}}}}"#,
            named(&attestation),
            declared(true, attestation.len()),
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
            beyond: None,
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
            beyond: None,
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
        r#"{{"schemaVersion":2,"layers":[{{"size":1,"annotations":{{"in-toto.io/predicate-type":"{SLSA}"}}}}]}}"#
    );
    let blobless_index = format!(
        r#"{{"manifests":[{{"digest":"{}","size":{},"annotations":{{"vnd.docker.reference.digest":"sha256:a","vnd.docker.reference.type":"attestation-manifest"}},"platform":{{"os":"unknown"}}}}]}}"#,
        named(&blobless),
        blobless.len()
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

/// What reading the provenance beside `routes`' index at `digest` came to.
async fn read_off(digest: &str, routes: Vec<(String, Response)>) -> Result<Built, String> {
    Oci::new(serving(routes))
        .built(&caddy(digest))
        .await
        .map_err(|why| why.reason)
}

/// Whether a read was answered as beyond what it holds, saying `said`, with no statement.
fn beyond(read: &Result<Built, String>, said: &str) -> bool {
    read.as_ref().is_ok_and(|built| {
        built.statements.is_empty()
            && built
                .beyond
                .as_deref()
                .is_some_and(|why| why.contains(said))
    })
}

#[tokio::test]
async fn an_index_listing_more_entries_than_a_read_holds_is_beyond_it() {
    let platforms: Vec<Platform> = (0..=super::MOST_ENTRIES)
        .map(|at| Platform {
            image: named(&format!("platform {at}")),
            layers: Vec::new(),
        })
        .collect();
    let (digest, routes) = published(&platforms);
    let read = read_off(&digest, routes).await;
    assert!(beyond(&read, "entries"), "{read:?}");
    let (digest, routes) = published(platforms.get(1..).unwrap_or_default());
    let read = read_off(&digest, routes).await;
    assert!(
        read.is_ok_and(|built| built.beyond.is_none() && built.images.len() == super::MOST_ENTRIES)
    );
}

#[tokio::test]
async fn an_attestation_listing_more_statements_than_a_read_holds_is_beyond_it() {
    let layers = (0..=super::MOST_STATEMENTS)
        .map(|at| (SLSA, format!("statement {at}")))
        .collect();
    let (digest, routes) = published(&[Platform {
        image: named("amd64"),
        layers,
    }]);
    let read = read_off(&digest, routes).await;
    assert!(beyond(&read, "provenance statements"), "{read:?}");
}

#[tokio::test]
async fn a_manifest_or_statement_declaring_more_than_a_read_holds_is_beyond_it_and_unread() {
    for (attestation, said) in [(true, "its attestation"), (false, "the statement")] {
        let (digest, routes) = published_as(
            &[Platform {
                image: named("amd64"),
                layers: vec![(SLSA, "the provenance".to_owned())],
            }],
            |names_attestation, size| {
                if names_attestation == attestation {
                    if attestation {
                        super::LARGEST_MANIFEST + 1
                    } else {
                        super::LARGEST_STATEMENT + 1
                    }
                } else {
                    size
                }
            },
        );
        let asked = serving(routes);
        let read = Oci::new(asked.clone())
            .built(&caddy(&digest))
            .await
            .map_err(|why| why.reason);
        assert!(beyond(&read, said), "{read:?}");
        assert!(
            !asked.asked_for(&named("the provenance")),
            "a statement past the cap was fetched"
        );
    }
}

#[tokio::test]
async fn statements_coming_to_more_than_a_read_holds_for_an_image_are_beyond_it() {
    let platforms: Vec<Platform> = (0..5)
        .map(|at| Platform {
            image: named(&format!("platform {at}")),
            layers: vec![(SLSA, format!("statement {at}"))],
        })
        .collect();
    let (digest, routes) = published_as(&platforms, |attestation, size| {
        if attestation {
            size
        } else {
            super::LARGEST_STATEMENT
        }
    });
    let read = read_off(&digest, routes).await;
    assert!(beyond(&read, "bytes a read holds"), "{read:?}");
}

#[tokio::test]
async fn a_body_longer_than_its_descriptor_declares_is_unreadable() {
    let (digest, routes) = published_as(
        &[Platform {
            image: named("amd64"),
            layers: vec![(SLSA, "the provenance".to_owned())],
        }],
        |attestation, size| if attestation { size } else { 3 },
    );
    let read = read_off(&digest, routes).await;
    assert!(
        read.as_ref()
            .is_err_and(|why| why.contains("more than 3 bytes")),
        "{read:?}"
    );
}

#[tokio::test]
async fn a_statement_listed_twice_is_read_once() {
    let (digest, routes) = published(&[Platform {
        image: named("amd64"),
        layers: vec![
            (SLSA, "the provenance".to_owned()),
            (SLSA, "the provenance".to_owned()),
        ],
    }]);
    let asked = serving(routes);
    let built = Oci::new(asked.clone()).built(&caddy(&digest)).await;
    assert_eq!(built.map(|built| built.statements.len()), Ok(1));
    let blob = named("the provenance");
    assert_eq!(
        asked
            .requests()
            .iter()
            .filter(|request| request.url.contains(&blob))
            .count(),
        1
    );
}
