use lemonfiber_plugin::Manifest;

use super::{Declaration, Secret};

/// A manifest declaring one of everything this record keeps: a claim of a core
/// capability and one of its own, a recipe reaching its own service twice and a
/// name outside the stack once — by a call and again by a pair — a secret, and an
/// override.
const DECLARING: &str = r#"
schema_version = 1

[plugin]
id          = "komga"
name        = "Komga"
version     = "1.2.0"
description = "Reads your comics on any browser"
without_it  = "Files on disk, no way to read them"
upstream    = "https://github.com/gotson/komga"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "komga"
name        = "Komga"
image       = "docker.io/gotson/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.11.0"
port        = 25600
bind        = "lan"
criticality = "important"

[[claim]]
capability = "media.serve"

[[claim]]
capability = "komga:kobo-sync"

[[recipe]]
id    = "adopt"
title = "t"
why   = "w"

[[recipe.step]]
id   = "in"
call = { method = "POST", to = "komga", path = "/api/v1/login" }

[[recipe.step]]
id   = "tell"
call = { method = "POST", to = "metadata.example.org", path = "/v1/series" }

[[recipe.step]]
id   = "again"
call = { method = "GET", to = "komga", path = "/api/v1/libraries" }

[[recipe.pair]]
value = "token"
to    = "metadata.example.org"

[[secret]]
id  = "api-key"
of  = "komga"
why = "Read the library counts"

[[override]]
id  = "homepage.services"
why = "Add its own entry"
"#;

/// Everything the record keeps is read off the manifest in the order it was
/// declared; a destination is recorded once however many ways it is reached; and
/// the plugin's own service is not somewhere it reaches.
#[test]
fn what_a_plugin_declared_is_kept_as_it_declared_it() {
    let declared = Manifest::from_toml(DECLARING)
        .ok()
        .map(|manifest| Declaration::of(&manifest));
    assert_eq!(
        declared,
        Some(Declaration {
            upstream: "https://github.com/gotson/komga".to_owned(),
            license: "MIT".to_owned(),
            reviewed: false,
            claims: vec!["media.serve".to_owned(), "komga:kobo-sync".to_owned()],
            overrides: vec![super::super::stating::Overriding {
                setting: "homepage.services".to_owned(),
                why: "Add its own entry".to_owned(),
            }],
            reaches: vec!["metadata.example.org".to_owned()],
            secrets: vec![Secret {
                id: "api-key".to_owned(),
                of: "komga".to_owned(),
                why: "Read the library counts".to_owned(),
            }],
        })
    );
}

/// A record written before any of this was kept reads as a plugin that declared
/// nothing, rather than being refused.
#[test]
fn a_record_that_kept_none_of_this_reads_as_declaring_nothing() {
    let read: Result<Declaration, _> = serde_json::from_str("{}");
    assert_eq!(read.ok(), Some(Declaration::default()));
}
