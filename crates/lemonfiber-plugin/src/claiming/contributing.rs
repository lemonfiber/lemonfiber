//! The rows a plugin adds to registers lemonfiber already runs.
//!
//! Adding is not overriding, and every rule here is that sentence in one of its
//! shapes: the point has to exist, the identity has to be the plugin's own, and
//! neither an identity nor a remedy's subject may be something the bundled set holds.
//!
//! Nothing here holds a list of fields. What a row carries is its point's to say, and
//! the points are published — so a point added later brings its own row with it and
//! these rules go on deciding, which is the whole reason they are published rather
//! than enumerated in first-party source.

use std::collections::{BTreeMap, BTreeSet};

use crate::extension::{self, Occupied};
use crate::schema::{Contribution, Manifest};
use crate::Violation;

use super::{listed, namespaced_with};

/// The rows a plugin adds to registers lemonfiber already runs.
///
/// Adding is not overriding, and every rule here is that sentence in one of its
/// shapes: the point has to exist, the identity has to be the plugin's own, and
/// neither an identity nor a remedy's subject may be something bundled.
pub(super) fn contributed(manifest: &Manifest, occupied: &[&str], found: &mut Vec<Violation>) {
    if manifest.contributions.is_empty() {
        return;
    }
    let published = extension::published(occupied);
    let points = listed(published.points.iter().map(|point| point.name));
    let asked: BTreeSet<&str> = manifest
        .requires
        .iter()
        .flat_map(|requires| &requires.capabilities)
        .map(String::as_str)
        .collect();

    let mut identities: BTreeSet<&str> = BTreeSet::new();
    let mut checks: BTreeSet<&str> = BTreeSet::new();
    let mut remedied: BTreeSet<&str> = BTreeSet::new();
    // Once per capability rather than once per row. A manifest contributing six checks
    // has one thing to fix, and six copies of a sentence is a list nobody reads to the
    // end of — which is where the rows it also has to fix are.
    let mut unasked: BTreeSet<&str> = BTreeSet::new();

    for entry in &manifest.contributions {
        let at = format!("contribution {}", entry.id);
        if !namespaced_with(&entry.id, &manifest.plugin.id) {
            found.push(Violation {
                location: format!("{at}.id"),
                message: format!(
                    "{} is not namespaced with this plugin's id ({}:…); a contribution is the \
                     plugin's and is attributed to it wherever it appears",
                    entry.id, manifest.plugin.id
                ),
            });
        }
        if !identities.insert(&entry.id) {
            found.push(Violation {
                location: at.clone(),
                message: format!("{} is declared twice", entry.id),
            });
        }
        let Some(point) = published.points.iter().find(|point| point.name == entry.at) else {
            found.push(Violation {
                location: format!("{at}.at"),
                message: format!(
                    "{} names no extension point this build publishes; it publishes: {points}",
                    entry.at
                ),
            });
            continue;
        };
        if !asked.contains(point.requires) {
            unasked.insert(point.requires);
        }
        bundled(entry, occupied, &at, found);
        row(entry, point, &at, found);

        // Asked of the published point rather than of a name written here. A rule that
        // recognised its subject by a literal would go on passing after a rename, with
        // nothing left in the set it was checking.
        if entry.at == extension::check() {
            checks.insert(&entry.id);
        }
        if let Some(about) = &entry.about {
            remedied.insert(about);
        }
    }

    for entry in &manifest.contributions {
        let Some(about) = &entry.about else { continue };
        if !checks.contains(about.as_str()) {
            found.push(Violation {
                location: format!("contribution {}.for", entry.id),
                message: format!(
                    "{about} names no check this manifest declares; a remedy for a check somebody \
                     else's plugin or the bundled set holds would be this plugin editing what \
                     lemonfiber says about a service it did not install"
                ),
            });
        }
    }

    for capability in &unasked {
        found.push(Violation {
            location: "requires.capabilities".to_owned(),
            message: format!(
                "a manifest contributing at one of this build's points asks for {capability} by \
                 name, so a lemonfiber that does not take contributions there refuses the \
                 manifest rather than reading the rows and dropping them"
            ),
        });
    }

    for check in checks.difference(&remedied) {
        found.push(Violation {
            location: format!("contribution {check}"),
            message:
                "carries no remedy; a check that can say something is wrong and nothing about \
                      what to do has moved the work to the operator rather than done it"
                    .to_owned(),
        });
    }
}

/// An identity a bundled row already holds, named wherever a contribution takes it.
///
/// Both the row's own identity and the check a remedy is for, because a remedy
/// pointing at a bundled check is a plugin replacing what lemonfiber says to do about
/// its own finding — which is the same collision wearing the other field's name.
///
/// A bundled identity ending in a dot is a family whose members are the operator's
/// configuration rather than this build's, so everything under it is taken too.
fn bundled(entry: &Contribution, occupied: &[&str], at: &str, found: &mut Vec<Violation>) {
    for (field, named) in [("id", Some(&entry.id)), ("for", entry.about.as_ref())] {
        let Some(named) = named else { continue };
        let Some(held) = occupied.iter().find(|held| {
            **held == named.as_str() || (held.ends_with('.') && named.starts_with(**held))
        }) else {
            continue;
        };
        found.push(Violation {
            location: format!("{at}.{field}"),
            message: format!(
                "{named} is the identity a bundled row already holds ({held}); adding is not \
                 overriding, and standing in for something bundled is an operator's recorded \
                 choice rather than a manifest's assertion"
            ),
        });
    }
}

/// One row, against the shape its own point publishes.
///
/// Nothing here holds a list of fields. What a row carries is the point's to say, so a
/// point added later brings its own row with it and this goes on deciding — which is
/// the whole reason the points are published rather than enumerated in source.
fn row(entry: &Contribution, point: &Occupied, at: &str, found: &mut Vec<Violation>) {
    let takes: BTreeSet<&str> = point
        .row
        .required
        .iter()
        .chain(point.row.optional)
        .copied()
        .collect();
    let carried = present(entry);

    for field in point.row.required {
        if !carried.contains_key(field) {
            found.push(Violation {
                location: at.to_owned(),
                message: format!("carries no {field}, which {} requires", point.name),
            });
        }
    }
    for field in carried.keys() {
        if takes.contains(field) {
            continue;
        }
        found.push(Violation {
            location: format!("{at}.{field}"),
            message: acting(field).map_or_else(
                || {
                    format!(
                        "{field} is outside what {} declares; it takes: {}",
                        point.name,
                        listed(takes.iter().copied())
                    )
                },
                |doing| {
                    format!(
                        "{field} would have this {doing}, and {} renders rather than runs; a \
                         remedy is text, and the ordered calls a repair that acted would make \
                         are declared as a [[recipe]] or not at all",
                        point.name
                    )
                },
            ),
        });
    }
    for closed in point.row.enums {
        let Some(Held::Word(said)) = carried.get(closed.field) else {
            continue;
        };
        if !closed.values.contains(&said.as_str()) {
            found.push(Violation {
                location: format!("{at}.{}", closed.field),
                message: format!(
                    "{said} is not one {} recognises; it takes: {}",
                    closed.field,
                    listed(closed.values.iter().copied())
                ),
            });
        }
    }
    for bound in point.row.bounds {
        let Some(Held::Number(number)) = carried.get(bound.field) else {
            continue;
        };
        if *number < bound.limits.min || *number > bound.limits.max {
            found.push(Violation {
                location: format!("{at}.{}", bound.field),
                message: format!(
                    "{number} is outside the bounds {} sets: {} to {}",
                    point.name, bound.limits.min, bound.limits.max
                ),
            });
        }
    }
}

/// What a field would have a row do, where the field is one that does rather than says.
///
/// One field, because one is all there is: a row makes a call by carrying the call, and
/// everything else it carries is a value that renders. A point whose row does not take
/// this field is a point whose engine reads rather than acts, so the refusal is a fact
/// about the row against the point rather than a list of points written down here.
///
/// It earns its own sentence away from the ordinary out-of-set refusal because the two
/// are different mistakes. Naming a field the point does not declare is usually a
/// typo or a row written for the wrong point; this one is somebody reaching for a
/// repair that acts, which is a real thing to want and has somewhere to go.
fn acting(field: &str) -> Option<&'static str> {
    (field == "request").then_some("ask the operator's system something")
}

/// What one field of a row holds, in the two kinds a published point constrains.
///
/// A closed set is about words and a bound is about numbers, so a row's fields are
/// read back as one or the other and everything else is present without being either.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Held {
    /// A word, which is what a closed set draws from.
    Word(String),
    /// A number, which is what a bound is on.
    Number(u32),
    /// Something the published constraints say nothing about.
    Other,
}

/// Every field this contribution actually declares, by the name the manifest spells.
///
/// The one table in this module, and it exists because the type is a union of every
/// point's row: what a *row* may carry is the point's to say, and what a
/// *contribution* may carry at all is the format's. A field added to the type and left
/// off here would be carried into a register without any point having declared it, so
/// a test below holds the two to each other.
pub(super) fn present(entry: &Contribution) -> BTreeMap<&'static str, Held> {
    let mut carried: BTreeMap<&'static str, Held> = BTreeMap::new();
    carried.insert("id", Held::Word(entry.id.clone()));
    for (field, word) in [
        ("title", &entry.title),
        ("category", &entry.category),
        ("why", &entry.why),
        ("fixture", &entry.fixture),
        ("service", &entry.service),
        ("for", &entry.about),
        ("action", &entry.action),
        ("detail", &entry.detail),
    ] {
        if let Some(word) = word {
            carried.insert(field, Held::Word(word.clone()));
        }
    }
    if let Some(seconds) = entry.timeout_s {
        carried.insert("timeout_s", Held::Number(seconds));
    }
    if entry.request.is_some() {
        carried.insert("request", Held::Other);
    }
    if entry.expect.is_some() {
        carried.insert("expect", Held::Other);
    }
    carried
}

#[cfg(test)]
mod tests {
    use super::contributed;
    use crate::{Manifest, Violation};

    /// The identities the doctor's register holds in these tests.
    const OCCUPIED: &[&str] = &["storage.space", "environment.engine"];

    /// A plugin declaring one check and the remedy that check carries.
    ///
    /// The shape every case below varies one thing in, so what a case is about is the
    /// line it changed rather than the fifty it repeated.
    const DECLARING: &str = r#"
schema_version = 1

[plugin]
id          = "komga"
name        = "Komga"
version     = "1.1.0"
description = "Reads comics in a browser"
without_it  = "Comics stay folders of images"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "komga"
name        = "Komga"
image       = "example.invalid/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.26.3"
port        = 25600
bind        = "lan"
criticality = "enhancing"

[requires]
capabilities = ["doctor.contribute"]

[[contribution]]
at        = "doctor.check"
id        = "komga:claimed"
title     = "Komga has an administrator"
category  = "credentials"
request   = { method = "GET", path = "/api/v1/claim" }
expect    = { status = 200, json = { isClaimed = true } }
fixture   = "fixtures/claim.json"
why       = "An unclaimed Komga hands administrator to whoever asks first."

[[contribution]]
at     = "doctor.remedy"
id     = "komga:claim-it"
for    = "komga:claimed"
action = "Open Komga and create the administrator account"
why    = "Until somebody does, the first caller on the household network becomes it."
"#;

    /// What the rules say about a manifest, as one sentence per violation.
    fn against(text: &str) -> Vec<String> {
        // A fixture this build cannot read is a mistake in the test rather than a
        // refusal, and it has to stop the case rather than come back as an empty list
        // that every "says nothing" assertion below would pass on.
        let read = Manifest::from_toml(text);
        assert!(read.is_ok(), "the fixture does not read: {read:?}");
        let mut found: Vec<Violation> = Vec::new();
        if let Ok(manifest) = read {
            contributed(&manifest, OCCUPIED, &mut found);
        }
        found.iter().map(ToString::to_string).collect()
    }

    /// Whether some one sentence carries all of these.
    fn says(found: &[String], words: &[&str]) -> bool {
        found
            .iter()
            .any(|said| words.iter().all(|word| said.contains(word)))
    }

    /// The fixture itself is refused for nothing, so every case below means something.
    ///
    /// Without this, a rule that refused every manifest would pass every case that
    /// asserts on what was refused, and the cases asserting silence would be the only
    /// ones telling the truth.
    #[test]
    fn a_plugin_declaring_a_check_and_its_remedy_is_refused_for_nothing() {
        assert_eq!(against(DECLARING), Vec::<String>::new());
    }

    /// A remedy that would call a service is refused as a remedy, and told where to go.
    ///
    /// The mistake is a real thing to want — a repair that puts the fault right rather
    /// than describing it — so the refusal names the block that carries one instead of
    /// answering that the field is not on the list.
    #[test]
    fn a_remedy_that_would_act_on_the_operator_s_system_is_refused_as_a_remedy() {
        let acting = DECLARING.replace(
            r#"action = "Open Komga and create the administrator account""#,
            "action = \"Claim it\"\nrequest = { method = \"POST\", path = \"/api/v1/claim\" }",
        );
        let said = against(&acting);
        assert!(
            says(&said, &["komga:claim-it", "request", "[[recipe]]"]),
            "{said:?}"
        );
        assert!(
            says(&said, &["rendered", "doctor.remedy"])
                || says(&said, &["renders", "doctor.remedy"]),
            "the refusal says which point renders rather than runs: {said:?}"
        );
    }

    /// And the ordinary out-of-set refusal still happens, for a field that only says.
    ///
    /// The half that keeps the new sentence from swallowing the old one. A row carrying
    /// a field that belongs to the other point is usually a row written for the wrong
    /// point, and telling its author about recipes would send them somewhere unhelpful.
    #[test]
    fn a_field_that_says_rather_than_does_is_refused_without_mentioning_recipes() {
        let misplaced = DECLARING.replace(
            r#"action = "Open Komga and create the administrator account""#,
            "action = \"Open Komga\"\ncategory = \"credentials\"",
        );
        let said = against(&misplaced);
        assert!(
            says(&said, &["category", "outside what", "doctor.remedy"]),
            "{said:?}"
        );
        assert!(
            !says(&said, &["[[recipe]]"]),
            "a field that only says is not a repair that acts: {said:?}"
        );
    }

    /// A check still requires the very field a remedy is refused for carrying.
    ///
    /// The direction a rule keyed on the field name alone would get wrong: `request` is
    /// how a check asks, and a refusal that fired wherever it appeared would make the
    /// check undeclarable.
    #[test]
    fn the_field_a_remedy_may_not_carry_is_the_one_a_check_must() {
        let asking_nothing = DECLARING.replace(
            r#"request   = { method = "GET", path = "/api/v1/claim" }"#,
            "",
        );
        let said = against(&asking_nothing);
        assert!(
            says(&said, &["carries no request", "doctor.check"]),
            "{said:?}"
        );
        assert!(!says(&said, &["[[recipe]]"]), "{said:?}");
    }
}
