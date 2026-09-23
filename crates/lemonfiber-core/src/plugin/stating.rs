//! What installing a plugin says it will do, in the terms an operator decides on.
//!
//! A rehearsal is worth having only if it is the account the install then follows, so
//! nothing here is built for the rehearsal: it is the same derivation, read off the
//! same manifest and the same list of writes, on the run that reports and on the run
//! that acts. The tense is the surface's business; the facts are one set.
//!
//! **What is stated is what lands on the machine, not what is journalled.** The two
//! lists are nearly the same and differ in the one place that matters to a person
//! being asked to agree: a directory that is already there is not written and not
//! recorded, and a document left behind by an install that did not finish is
//! overwritten without being recorded — because recording it as *made* would have a
//! reversal remove a file this run did not create. The second of those is a change to
//! the machine and belongs here; the record's job is different, and it is the record
//! that gets to be narrower.
//!
//! Nothing here touches a disk. Every answer is a function of the manifest and of the
//! list [`super::placing::writes`] derives, so what an install would say can be put in
//! front of a test without a stack directory under it.

use lemonfiber_plugin::Manifest;
use serde::Serialize;

use super::claimed::Verdict;
use super::placing::Write;

/// What an install puts at one path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(rename = "PluginPuts")]
pub enum Puts {
    /// A directory brought into being, which is where one service keeps its own
    /// configuration.
    Directory,
    /// A document written, which is the container lemonfiber derives for the plugin.
    Document,
}

/// One change installing a plugin makes to the machine.
///
/// A path and what goes at it, which is the whole of what an install touches: a
/// plugin's wiring goes in files of its own, so there is no change here that is an
/// edit to something somebody else owns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "PluginChange")]
pub struct Changing {
    /// Where it lands, in full.
    ///
    /// In full rather than relative to the stack, because *what is this about to do
    /// to my machine* is answered by a path somebody can go and look at — and a
    /// relative one is right about a directory the reader has to work out for
    /// themselves.
    pub path: String,
    /// What lands there.
    pub puts: Puts,
}

/// One proof that has to hold before a plugin is reported installed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "PluginProving")]
pub struct Proving {
    /// The proof's id, which its verdict is reported against.
    pub proof: String,
    /// What it establishes, in one line.
    pub establishes: String,
    /// Which of the plugin's services it asks, where the manifest settles that.
    ///
    /// Nothing where it does not, which is a manifest the reader has already refused
    /// — carried as an absence rather than as a guess, so that a report built from a
    /// manifest nobody held to the reader says *this was not settled* instead of
    /// naming whichever service came first.
    pub of: Option<String>,
    /// What it asks, as the method and the path it is asked at.
    pub asks: String,
    /// Why it is worth asserting.
    pub why: String,
    /// What asking it came to, or nothing where it was not asked.
    ///
    /// Absent on a rehearsal, which asks nothing. That is a different fact from a
    /// proof that was asked and established nothing, and the two must not read alike:
    /// one is an account of what would happen, the other is a service that did not
    /// answer.
    pub came_to: Option<Verdict>,
}

/// One bundled thing a plugin declares it will change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "PluginOverriding")]
pub struct Overriding {
    /// Which bundled setting it changes.
    pub setting: String,
    /// What changing it is for.
    pub why: String,
}

/// Every change installing this plugin makes, in the order it makes them.
///
/// Read off the very list the writes are carried out from, so the account and the act
/// cannot come apart. A second walk over the record would be a second derivation, free
/// to state a path the install does not write.
#[must_use]
pub fn changes(planned: &[Write]) -> Vec<Changing> {
    planned
        .iter()
        .map(|write| Changing {
            path: write.path.display().to_string(),
            puts: if write.is_directory() {
                Puts::Directory
            } else {
                Puts::Document
            },
        })
        .collect()
}

/// Every proof the install would run, in the order the manifest declares them.
///
/// Stated rather than run. What a rehearsal owes an operator is *which questions will
/// be asked of what*, before there is anything to ask them of; the verdicts are the
/// install's own business and are reached where the asking happens.
#[must_use]
pub fn proofs(manifest: &Manifest) -> Vec<Proving> {
    manifest
        .proofs
        .iter()
        .map(|proof| Proving {
            proof: proof.id.clone(),
            establishes: proof.title.clone(),
            of: manifest
                .asks(proof.service.as_deref())
                .map(|service| service.id.clone()),
            asks: format!("{} {}", proof.request.method, proof.request.path),
            why: proof.why.clone(),
            came_to: None,
        })
        .collect()
}

/// Every bundled thing the plugin declares it will change.
///
/// **This is the full extent rather than a sample of it, and that is a property of the
/// reader rather than a promise made here.** A manifest may change a bundled setting
/// only through a recipe, and a recipe reaching a setting no `[[override]]` names is
/// refused before anything is installed — so what is declared is what can happen, and
/// an empty list means a plugin that changes nothing of the stack's.
#[must_use]
pub fn overrides(manifest: &Manifest) -> Vec<Overriding> {
    manifest
        .overrides
        .iter()
        .map(|one| Overriding {
            setting: one.id.clone(),
            why: one.why.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use lemonfiber_plugin::Manifest;

    use super::{changes, overrides, proofs, Changing, Overriding, Proving, Puts};
    use crate::plugin::installed::Installed;

    /// A manifest declaring one service, one proof and one override.
    const MANIFEST: &str = r#"
schema_version = 1

[plugin]
id          = "komga"
name        = "Komga"
version     = "1.2.0"
description = "Reads your comics on any browser"
without_it  = "Files on disk, no way to read them"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "komga"
name        = "Komga"
image       = "example.invalid/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.11.0"
port        = 25600
bind        = "lan"
criticality = "important"
takes_data  = true
config_path = "/app/data"

[[proof]]
id      = "answers"
title   = "the library API answers"
why     = "a plugin whose service does not answer is not installed"
fixture = "fixtures/libraries.json"
request = { method = "GET", path = "/api/v1/libraries" }
expect  = { status = 200 }

[[override]]
id  = "seerr.settings"
why = "a request for a comic has to reach the library that holds comics"
"#;

    /// The stack these are written beneath.
    fn stack() -> &'static Path {
        Path::new("/opt/lemonfiber/stack")
    }

    /// The fixture, with whatever departure the case under test needs made to it
    /// first.
    ///
    /// Nothing where the fixture stopped being a manifest, so a fixture that broke
    /// fails the assertion it was written for rather than somewhere further down.
    fn manifest(change: impl FnOnce(&mut Manifest)) -> Option<Manifest> {
        let mut read = Manifest::from_toml(MANIFEST).ok()?;
        change(&mut read);
        Some(read)
    }

    /// The fixture as its author wrote it.
    fn whole() -> Option<Manifest> {
        manifest(|_| ())
    }

    /// What installing the fixture writes, beneath the stack above.
    fn planned() -> Vec<crate::plugin::Write> {
        whole()
            .map(|read| crate::plugin::writes(&Installed::of(&read), stack()))
            .unwrap_or_default()
    }

    /// What a manifest states it would change.
    fn stated(read: Option<&Manifest>) -> Vec<Proving> {
        read.map(proofs).unwrap_or_default()
    }

    #[test]
    fn every_write_is_stated_as_a_path_and_what_goes_at_it() {
        assert_eq!(
            changes(&planned()),
            vec![
                Changing {
                    path: "/opt/lemonfiber/stack/config/komga".to_owned(),
                    puts: Puts::Directory,
                },
                Changing {
                    path: "/opt/lemonfiber/stack/compose/plugins/komga.yml".to_owned(),
                    puts: Puts::Document,
                },
            ]
        );
    }

    /// Stated in the order the install makes them, which is the order a reversal
    /// walks backwards — so what an operator reads and what a reversal would do are
    /// one list read in opposite directions.
    #[test]
    fn the_changes_are_stated_in_the_order_the_install_makes_them() {
        let planned = planned();
        assert_eq!(
            changes(&planned)
                .iter()
                .map(|one| one.path.clone())
                .collect::<Vec<String>>(),
            planned
                .iter()
                .map(|write| write.path.display().to_string())
                .collect::<Vec<String>>()
        );
    }

    #[test]
    fn a_proof_is_stated_with_what_it_asks_of_which_service_and_why() {
        assert_eq!(
            stated(whole().as_ref()),
            vec![Proving {
                proof: "answers".to_owned(),
                establishes: "the library API answers".to_owned(),
                of: Some("komga".to_owned()),
                asks: "GET /api/v1/libraries".to_owned(),
                why: "a plugin whose service does not answer is not installed".to_owned(),
                came_to: None,
            }]
        );
    }

    /// A plugin that proves nothing and changes nothing of the stack's states
    /// neither, which is what lets a surface say so rather than print an empty
    /// heading.
    #[test]
    fn a_plugin_that_proves_nothing_and_overrides_nothing_states_neither() {
        let bare = manifest(|read| {
            read.proofs.clear();
            read.overrides.clear();
        });
        assert!(stated(bare.as_ref()).is_empty());
        assert!(bare.as_ref().map(overrides).unwrap_or_default().is_empty());
    }

    #[test]
    fn a_declared_override_is_stated_with_what_changing_it_is_for() {
        assert_eq!(
            whole().as_ref().map(overrides).unwrap_or_default(),
            vec![Overriding {
                setting: "seerr.settings".to_owned(),
                why: "a request for a comic has to reach the library that holds comics".to_owned(),
            }]
        );
    }

    /// A proof naming a service the manifest does not declare settles nothing, and
    /// says so rather than naming whichever service came first. The reader refuses
    /// such a manifest, so this is what a report built from one nobody held to it
    /// says.
    #[test]
    fn a_proof_that_does_not_settle_which_service_it_asks_names_none() {
        let elsewhere = manifest(|read| {
            for proof in &mut read.proofs {
                proof.service = Some("nowhere".to_owned());
            }
        });
        assert_eq!(
            stated(elsewhere.as_ref())
                .into_iter()
                .next()
                .and_then(|one| one.of),
            None
        );
    }
}
