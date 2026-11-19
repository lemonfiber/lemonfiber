use lemonfiber_plugin::BuiltOn;
use lemonfiber_ports::registry::{Built, Image, Statement, Unanswerable};

use super::{based, Basis};

const AMD64: &str = "sha256:1e9c97cdf2d474af28ce81c8517264524edfdf47b44dfe8b9d1d802177e20c2e";
const ARM64: &str = "sha256:71b3c15c8a14b6dd713318886b1a705e25d124afce2577dbe6ab16e9070c4841";
const BASE: &str = "504a44d7d74bd2bc28a66afc4774912fc21223ca8a67734af1c54ab2f999b489";
const SCANNER: &str = "ae4f3b554449e7e25548e7d8ccc029d17357348e30c6e3df01b92bc93654d6a9";

fn caddy(release: &str) -> BuiltOn {
    BuiltOn {
        image: "caddy".to_owned(),
        release: release.to_owned(),
    }
}

/// One dependency a statement lists, by its purl and the sha256 beside it.
fn dependency(uri: &str, hex: &str) -> String {
    format!(r#"{{"uri":"{uri}","digest":{{"sha256":"{hex}"}}}}"#)
}

/// A SLSA v1 statement about `image`, listing `dependencies` as resolved.
fn statement(image: &str, dependencies: &[String]) -> Statement {
    let subject = image.strip_prefix("sha256:").unwrap_or(image);
    Statement {
        attests: image.to_owned(),
        body: format!(
            r#"{{"_type":"https://in-toto.io/Statement/v1","predicateType":"https://slsa.dev/provenance/v1","subject":[{{"name":"pkg:docker/ghcr.io/lemonfiber/caddy@latest","digest":{{"sha256":"{subject}"}}}}],"predicate":{{"buildDefinition":{{"resolvedDependencies":[{}]}}}}}}"#,
            dependencies.join(",")
        ),
    }
}

/// What the caddy build's statements list for `platform`: the scanner, and the base.
fn caddy_built(platform: &str, release: &str) -> Statement {
    statement(
        platform,
        &[
            dependency(
                &format!(
                    "pkg:docker/docker/buildkit-syft-scanner@1.12.0?digest=sha256:{SCANNER}&platform=linux%2Famd64"
                ),
                SCANNER,
            ),
            dependency(
                &format!("pkg:docker/caddy@{release}?digest=sha256:{BASE}&platform=linux%2Famd64"),
                BASE,
            ),
        ],
    )
}

fn held(built_on: &BuiltOn, built: Built) -> Basis {
    based("caddy", built_on, &Ok(built)).held
}

#[test]
fn a_base_every_platforms_build_is_recorded_as_starting_from_is_recorded() {
    let built = Built {
        images: vec![AMD64.to_owned(), ARM64.to_owned()],
        statements: vec![caddy_built(AMD64, "2.11.7"), caddy_built(ARM64, "2.11.7")],
    };
    let read = based("caddy", &caddy("2.11.7"), &Ok(built));
    assert_eq!(
        (read.image.as_str(), read.release.as_str()),
        ("caddy", "2.11.7")
    );
    assert_eq!(
        read.held,
        Basis::Recorded {
            digests: vec![format!("sha256:{BASE}")]
        }
    );
    assert!(!read.held.refuses());
}

#[test]
fn a_release_the_build_did_not_start_from_is_refused_naming_the_service_and_what_was_recorded() {
    let built = Built {
        images: vec![AMD64.to_owned()],
        statements: vec![caddy_built(AMD64, "2.11.7")],
    };
    let refused = held(&caddy("2.11.8"), built);
    assert!(refused.refuses());
    let Basis::Refused { why } = refused else {
        unreachable!("{refused:?}")
    };
    for named in [
        "caddy says it is built on caddy 2.11.8",
        AMD64,
        &format!("caddy 2.11.7 at sha256:{BASE}"),
        &format!("docker/buildkit-syft-scanner 1.12.0 at sha256:{SCANNER}"),
    ] {
        assert!(why.contains(named), "{named} in {why}");
    }
}

#[test]
fn a_platform_whose_build_is_not_recorded_starting_from_it_refuses_the_whole() {
    let elsewhere = Built {
        images: vec![AMD64.to_owned(), ARM64.to_owned()],
        statements: vec![caddy_built(AMD64, "2.11.7"), caddy_built(ARM64, "2.11.6")],
    };
    let unattested = Built {
        images: vec![AMD64.to_owned(), ARM64.to_owned()],
        statements: vec![caddy_built(AMD64, "2.11.7")],
    };
    for (built, said) in [
        (elsewhere, "caddy 2.11.6 at"),
        (unattested, "records no image the build started from"),
    ] {
        let Basis::Refused { why } = held(&caddy("2.11.7"), built) else {
            unreachable!("{said}")
        };
        assert!(why.contains(ARM64) && why.contains(said), "{why}");
        assert!(!why.contains(AMD64), "{why}");
    }
    let Basis::Refused { why } = held(&caddy("2.11.7"), Built::default()) else {
        unreachable!("an index listing nothing was recorded")
    };
    assert!(why.contains("lists no image a platform runs"), "{why}");
}

#[test]
fn a_statement_about_another_image_or_unreadable_records_nothing_and_says_so() {
    let mut another = caddy_built(ARM64, "2.11.7");
    another.attests = AMD64.to_owned();
    let unreadable = Statement {
        attests: AMD64.to_owned(),
        body: "not json".to_owned(),
    };
    for (statement, said) in [
        (another, "about another image"),
        (unreadable, "is not readable"),
    ] {
        let built = Built {
            images: vec![AMD64.to_owned()],
            statements: vec![statement],
        };
        let Basis::Refused { why } = held(&caddy("2.11.7"), built) else {
            unreachable!("{said}")
        };
        assert!(
            why.contains(said) && why.contains("records no image the build started from"),
            "{why}"
        );
    }
}

#[test]
fn provenance_nobody_could_ask_for_is_unproven_rather_than_refused() {
    let asked = Image::new("ghcr.io/lemonfiber/caddy", AMD64);
    let read = based(
        "caddy",
        &caddy("2.11.7"),
        &Err(Unanswerable::about(&asked, "connection refused")),
    );
    let Basis::Unproven { why } = &read.held else {
        unreachable!("{read:?}")
    };
    assert!(
        why.contains("connection refused") && why.contains("caddy"),
        "{why}"
    );
    assert!(!read.held.refuses());
}

#[test]
fn an_image_on_the_default_registry_is_the_same_image_however_it_is_named() {
    let materials = Statement {
        attests: AMD64.to_owned(),
        body: format!(
            r#"{{"subject":[{{"digest":{{"sha256":"{}"}}}}],"predicate":{{"materials":[{}]}}}}"#,
            AMD64.trim_start_matches("sha256:"),
            dependency("pkg:docker/library/caddy@2.11.7", BASE)
        ),
    };
    for image in ["caddy", "docker.io/library/caddy", "index.docker.io/caddy"] {
        let built_on = BuiltOn {
            image: image.to_owned(),
            release: "2.11.7".to_owned(),
        };
        let built = Built {
            images: vec![AMD64.to_owned()],
            statements: vec![materials.clone()],
        };
        assert!(
            matches!(held(&built_on, built), Basis::Recorded { .. }),
            "{image}"
        );
    }
}

#[test]
fn a_dependency_with_no_digest_or_two_that_disagree_is_no_image_the_build_started_from() {
    let other = "0".repeat(64);
    for listed in [
        dependency(
            &format!("pkg:docker/caddy@2.11.7?digest=sha256:{other}"),
            BASE,
        ),
        r#"{"uri":"pkg:docker/caddy@2.11.7"}"#.to_owned(),
        dependency("pkg:docker/caddy@2.11.7", "not hex"),
        dependency("pkg:docker/caddy", BASE),
        dependency("pkg:docker/caddy@2.11%ZZ", BASE),
        dependency("pkg:github/caddyserver/caddy@2.11.7", BASE),
    ] {
        let built = Built {
            images: vec![AMD64.to_owned()],
            statements: vec![statement(AMD64, std::slice::from_ref(&listed))],
        };
        assert!(
            held(&caddy("2.11.7"), built).refuses(),
            "{listed} was read as the base"
        );
    }
}

#[test]
fn a_release_written_in_escapes_is_the_release_it_spells() {
    let built = Built {
        images: vec![AMD64.to_owned()],
        statements: vec![statement(
            AMD64,
            &[dependency("pkg:docker/caddy@2.11.7%2Bbuild", BASE)],
        )],
    };
    assert!(matches!(
        held(&caddy("2.11.7+build"), built),
        Basis::Recorded { .. }
    ));
}
