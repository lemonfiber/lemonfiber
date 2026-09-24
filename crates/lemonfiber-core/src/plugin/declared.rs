//! What a plugin said about itself when it was installed, kept for as long as it is.
//!
//! Beside what the install decided rather than inside it, because the two are
//! different kinds of fact: where a container is published is a decision this build
//! took, and which hosts a plugin says it will reach is a declaration the plugin made
//! and this build held it to. Both are read off the manifest once, at install, and kept
//! here — because the author's directory may be gone the moment the install is done,
//! and the one read of what each plugin is doing has to answer about this machine
//! rather than about a document.
//!
//! **Every field is defaulted**, so a record written before these were kept reads as a
//! plugin that declared nothing, which is the answer that invents nothing — and says
//! so, rather than being refused.

use lemonfiber_plugin::Manifest;
use serde::{Deserialize, Serialize};

use super::stating::Overriding;

/// One credential a plugin says it will hold, without a value and with no place for one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginSecret")]
pub struct Secret {
    /// What the value is, within the plugin.
    pub id: String,
    /// Whose credential it is.
    pub of: String,
    /// What holding it is for.
    pub why: String,
}

/// What a plugin declared about itself, as its install read it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginDeclaration")]
pub struct Declaration {
    /// Where its source is published, as the plugin names it.
    #[serde(default)]
    pub upstream: String,
    /// The licence it is distributed under.
    #[serde(default)]
    pub license: String,
    /// Whether anybody reviewed it before it was installed.
    ///
    /// False for every install from a path an operator named — which is every install
    /// this build makes. Carried rather than left implicit, because an unreviewed
    /// plugin is to be said to be one for as long as it is installed, and a field that
    /// is only ever false today is still the field a reviewed install will set.
    #[serde(default)]
    pub reviewed: bool,
    /// Every capability it claims, core and its own, in the order it declares them.
    ///
    /// Apart from what its services fill: a claim is what it says it can do and has to
    /// demonstrate, and a capability of its own is claimed without anything asking for
    /// it.
    #[serde(default)]
    pub claims: Vec<String>,
    /// Every bundled setting it declares it may change.
    #[serde(default)]
    pub overrides: Vec<Overriding>,
    /// Every destination a recipe of its could reach that is not one of its own
    /// services: a service of this stack's, or a name outside it.
    ///
    /// Recorded as declared rather than sorted into the two here, because which names
    /// are this stack's is a question about the stack, and the stack a record is read
    /// against is the one on the machine when it is read.
    #[serde(default)]
    pub reaches: Vec<String>,
    /// Every credential it says it will hold.
    #[serde(default)]
    pub secrets: Vec<Secret>,
}

impl Declaration {
    /// What this manifest declares about its plugin.
    #[must_use]
    pub fn of(manifest: &Manifest) -> Self {
        let own = |to: &str| manifest.services.iter().any(|service| service.id == to);
        let mut reaches: Vec<String> = Vec::new();
        for to in manifest.recipes.iter().flat_map(|recipe| {
            recipe
                .steps
                .iter()
                .map(|step| step.call.to.as_str())
                .chain(recipe.pairs.iter().map(|pair| pair.to.as_str()))
        }) {
            if !own(to) && !reaches.iter().any(|held| held == to) {
                reaches.push(to.to_owned());
            }
        }
        Self {
            upstream: manifest.plugin.upstream.clone(),
            license: manifest.plugin.license.clone(),
            reviewed: false,
            claims: manifest
                .claims
                .iter()
                .map(|claim| claim.capability.clone())
                .collect(),
            overrides: super::stating::overrides(manifest),
            reaches,
            secrets: manifest
                .secrets
                .iter()
                .map(|secret| Secret {
                    id: secret.id.clone(),
                    of: secret.of.clone(),
                    why: secret.why.clone(),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
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
}
