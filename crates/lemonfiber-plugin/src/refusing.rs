//! What a manifest is refused for, once it is the shape the schema describes.
//!
//! [`crate::conforming`] answers whether a file is a manifest at all — the fields, the
//! kinds, the closed sets. What is left is everything the shape cannot say: that a
//! digest is a digest, that a path is one directory and not the library, that a
//! capability asked for is one this build has, that a name is not one the stack
//! already holds, that nothing is reached which was not declared. Those are rules over
//! values, and a generated schema is the wrong place for them: a schema an author's
//! editor enforces has to describe the reader exactly, and a reader that refused a
//! well-shaped digest for being the wrong length would be describing a rule rather
//! than a shape.
//!
//! Every one of them is reported in one pass with the others, because an author fixing
//! a third-party manifest one fault per run is guessing. Nothing here stops at the first
//! thing it finds.
//!
//! **A manifest is refused whole.** Nothing here applies part of one: the shape is
//! answered for before a value is looked at, and the values are answered for together,
//! so what a caller gets back is every reason this build would not act on the file
//! rather than the first one.
//!
//! **And refused rather than narrowed.** There is no route through here that drops
//! the part of a manifest it will not accept and applies the rest, and nowhere for
//! one to go: this reads a manifest and answers about it. A plugin installed with
//! the excess quietly removed would be running under a declaration that no longer
//! describes it, and the declaration is the entire basis on which a stranger's
//! contribution was judged.

mod bundled;
pub mod carried;
mod colliding;
mod evidence;
mod naming;
mod reaching;
mod recipes;

use std::collections::BTreeSet;

use crate::offering;
use crate::schema::{Manifest, Plugin, Service};
use crate::{claiming, Violation};

/// Everything this build refuses about a manifest, in one pass.
///
/// `occupied` is the identities lemonfiber's own registers already hold, passed in
/// rather than read here: a bundled check that is renamed has to move what a
/// contribution may collide with, and a copy kept here would go on reserving a name
/// nothing holds.
///
/// What the *stack* holds is read rather than passed, and the difference between the
/// two is which of them has a file. The doctor's register is assembled in code and
/// exists nowhere else; the stack description is the artefact this binary is built
/// from, so [`bundled`] reads that rather than keeping a second answer beside it.
///
/// An empty answer is a manifest this build would act on.
#[must_use]
pub fn refusals(manifest: &Manifest, occupied: &[&str]) -> Vec<Violation> {
    let mut found = claiming::violations(manifest, occupied);
    declaring(&manifest.plugin, &mut found);
    running(manifest, &mut found);
    colliding::with_the_stack(manifest, &mut found);
    reaching::beyond(manifest, &mut found);
    naming::wired(manifest, &mut found);
    naming::about(manifest, &mut found);
    carried::carried(manifest, &mut found);
    evidence::asking(manifest, &mut found);
    evidence::looking(manifest, &mut found);
    requiring(manifest, &mut found);
    recipes::declared(manifest, &mut found);
    readable(manifest, &mut found);
    found
}

/// Who the plugin says it is, and whether an operator could go and check.
///
/// The licence is recorded rather than constrained: every bundled service is
/// OSI-licensed because the bundled set is a list this project stands behind, and a
/// plugin is the operator's own choice. Refusing to install proprietary software on
/// somebody else's machine would be the tool standing between an operator and their
/// stack. An absent one is a different matter — it is the operator not being told.
fn declaring(plugin: &Plugin, found: &mut Vec<Violation>) {
    let at = "plugin";
    if plugin.license.trim().is_empty() {
        found.push(Violation {
            location: format!("{at}.license"),
            message: "is blank; a licence is recorded and shown rather than constrained, and an \
                      operator choosing whether to run somebody else's software is owed the one \
                      fact that says what running it commits them to"
                .to_owned(),
        });
    }
    if !carried::is_plugin_id(&plugin.id) {
        found.push(Violation {
            location: format!("{at}.id"),
            message: format!(
                "{} is not a plain lowercase name; it is the namespace every capability and \
                 contribution this plugin declares is prefixed with, so a separator in it would \
                 make two different plugins able to write the same identity",
                plugin.id
            ),
        });
    }
    if !plugin.upstream.starts_with("https://") {
        found.push(Violation {
            location: format!("{at}.upstream"),
            message: format!(
                "{} is not an https address; it is how an operator judges the thing being \
                 installed rather than the wrapper around it, so it has to be somewhere they can \
                 go and look",
                plugin.upstream
            ),
        });
    }
    if plugin.forms.is_empty() {
        found.push(Violation {
            location: format!("{at}.forms"),
            message: "names no form, so the service it installs would join nothing and start with \
                      nothing"
                .to_owned(),
        });
    }
}

/// The prefix a digest carries, and the length of what follows it.
const DIGEST: (&str, usize) = ("sha256:", 64);

/// The mount every plugin's service gets, which its own directory may not be inside.
const DATA: &str = "/data";

/// What runs, and whether what runs is fixed.
///
/// At least one, because a plugin declaring none would leave the rest of the manifest
/// describing nothing. No ceiling, because a plugin is one thing to an operator and is
/// often two containers: a media server and the reader of its watch history are one
/// install and one uninstall, on two tiers, with two criticalities — and a format
/// permitting one forces the wider tier on both halves or splits the pair into two
/// plugins an operator keeps in step by hand.
///
/// Ids are unique within the manifest as well as across the stack. Two services sharing
/// one id is not a collision with anything installed, so the rule that catches it
/// elsewhere never fires; what it is is one name for two containers, and every later
/// rule that reaches for a service *by* name — a wiring, a proof, a contribution —
/// would reach the first of them and say nothing about the second.
fn running(manifest: &Manifest, found: &mut Vec<Violation>) {
    if manifest.services.is_empty() {
        found.push(Violation {
            location: "service".to_owned(),
            message: "declares no service, so there is nothing for the rest of this manifest to \
                      be about"
                .to_owned(),
        });
    }
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for service in &manifest.services {
        if !seen.insert(service.id.as_str()) {
            found.push(Violation {
                location: format!("service {}.id", service.id),
                message: "is declared twice, and everything that names a service by id would \
                          reach one of the two and say nothing about the other"
                    .to_owned(),
            });
        }
        pinned(service, found);
        placed(service, found);
    }
}

/// Whether the image reviewed and the image run are the same one.
///
/// A tag is not a pin. It is a name its publisher can repoint, so the thing somebody
/// read in a diff and the thing running on an operator's machine can differ with nothing
/// in the manifest changing. A digest can always be obtained, which is why its absence is
/// a fault in the manifest rather than a limitation of a registry.
fn pinned(service: &Service, found: &mut Vec<Violation>) {
    let at = format!("service {}", service.id);
    let (prefix, length) = DIGEST;
    if !carried::is_digest(&service.digest) {
        found.push(Violation {
            location: format!("{at}.digest"),
            message: format!(
                "{} is not a {prefix} digest of {length} hexadecimal characters, so what actually \
                 runs is not fixed by this manifest",
                service.digest
            ),
        });
    }
    if service.image.contains('@') || service.image.rsplit('/').next().is_some_and(has_tag) {
        found.push(Violation {
            location: format!("{at}.image"),
            message: format!(
                "{} carries its own tag or digest; the registry path is declared here and what \
                 runs is declared once, in `digest`, so a second pin here could disagree with it",
                service.image
            ),
        });
    }
    if service.tag.trim().is_empty() {
        found.push(Violation {
            location: format!("{at}.tag"),
            message: "is blank; the digest says what runs and the tag is the readable name beside \
                      it, without which a diff shows sixty-four characters and no version"
                .to_owned(),
        });
    }
    if service.port.is_some() && service.bind.is_none() {
        found.push(Violation {
            location: format!("{at}.bind"),
            message: "is not declared and a port is; the tier is what decides whether the service \
                      is reachable by name, and lemonfiber assigns the address from it"
                .to_owned(),
        });
    }
}

/// Whether a registry path carries a tag after its last separator.
///
/// Read after the last `/` on purpose: a registry host may carry a port, and `:5000` in
/// `localhost:5000/komga` is where that host answers rather than which version runs.
fn has_tag(last: &str) -> bool {
    last.contains(':')
}

/// Where the service's one configuration directory lands inside its container.
///
/// The number of mounts and their sources are lemonfiber's, and that is what makes what
/// a plugin can reach answerable from the format. Only the target is the plugin's, and
/// it is checked: a target inside the library would be a second mount over the
/// operator's media wearing a different name.
fn placed(service: &Service, found: &mut Vec<Violation>) {
    let Some(path) = &service.config_path else {
        return;
    };
    let at = format!("service {}.config_path", service.id);
    let inside_data = path == DATA || path.starts_with(&format!("{DATA}/"));
    if !path.starts_with('/') || path == "/" || path.contains("..") || !carried::is_directory(path)
    {
        found.push(Violation {
            location: at,
            message: format!(
                "{path} is not one plain absolute directory; what is permitted is a single \
                 absolute path that is not the root, with no `..`, written in letters, digits \
                 and `._/-` alone"
            ),
        });
        return;
    }
    if inside_data {
        found.push(Violation {
            location: at,
            message: format!(
                "{path} is inside {DATA}, which is the library mount; a configuration directory \
                 there would be a second mount over the operator's media under another name"
            ),
        });
    }
}

/// What the plugin needs of lemonfiber, against what this build has.
///
/// Answered by name and never by a version. A version number conflates *older* with
/// *missing something you needed*, so a plugin that stops working is told the wrong
/// thing: the message has to say which mechanism went.
fn requiring(manifest: &Manifest, found: &mut Vec<Violation>) {
    let offered = offering::offered();
    let listed = offered.iter().copied().collect::<Vec<&str>>().join(", ");
    let asked = manifest
        .requires
        .iter()
        .flat_map(|requires| &requires.capabilities);
    for name in asked {
        if offering::offers(name) {
            continue;
        }
        found.push(Violation {
            location: "requires.capabilities".to_owned(),
            message: format!(
                "{name} is not something this build offers a plugin; what it offers is: {listed}"
            ),
        });
    }
}

/// Whether everything declared can be read by the person reviewing the diff.
///
/// A manifest is judged before it is trusted, and judging it means reading it. A value
/// carrying something a diff cannot show is content that has to be executed or decoded
/// to be understood, which is the opposite of what makes a stranger's contribution
/// reviewable at all.
fn readable(manifest: &Manifest, found: &mut Vec<Violation>) {
    for (at, text) in declared(manifest) {
        if let Some(hidden) = text
            .chars()
            .find(|letter| letter.is_control() && *letter != '\n' && *letter != '\t')
        {
            found.push(Violation {
                location: at,
                message: format!(
                    "carries U+{:04X}, which a diff cannot show; everything a manifest declares \
                     has to be readable by whoever is deciding whether to trust it",
                    u32::from(hidden)
                ),
            });
        }
    }
}

/// Every string a manifest declares, with where it was declared.
///
/// Gathered rather than each rule reaching for the ones it cares about, because the
/// question here is about all of them: one field left out is one place something
/// unreadable can be written.
fn declared(manifest: &Manifest) -> Vec<(String, &str)> {
    let plugin = &manifest.plugin;
    let mut every: Vec<(String, &str)> = vec![
        ("plugin.name".to_owned(), plugin.name.as_str()),
        ("plugin.version".to_owned(), plugin.version.as_str()),
        ("plugin.description".to_owned(), plugin.description.as_str()),
        ("plugin.without_it".to_owned(), plugin.without_it.as_str()),
        ("plugin.license".to_owned(), plugin.license.as_str()),
        ("plugin.upstream".to_owned(), plugin.upstream.as_str()),
    ];
    for service in &manifest.services {
        let at = format!("service {}", service.id);
        every.push((format!("{at}.id"), service.id.as_str()));
        every.push((format!("{at}.name"), service.name.as_str()));
        every.push((format!("{at}.image"), service.image.as_str()));
        every.push((format!("{at}.tag"), service.tag.as_str()));
        if let Some(path) = &service.config_path {
            every.push((format!("{at}.config_path"), path.as_str()));
        }
    }
    for (at, wiring) in manifest.wirings.iter().enumerate() {
        let at = wiring.service.as_deref().map_or_else(
            || format!("wiring #{}", at + 1),
            |id| format!("wiring {id}"),
        );
        for (field, text) in [
            ("hostname", wiring.hostname.as_deref()),
            ("dashboard_group", wiring.dashboard_group.as_deref()),
        ] {
            if let Some(text) = text {
                every.push((format!("{at}.{field}"), text));
            }
        }
    }
    for claim in &manifest.claims {
        for probe in &claim.probes {
            every.push((
                format!("claim {} probe {}.request.path", claim.capability, probe.id),
                probe.request.path.as_str(),
            ));
        }
    }
    for proof in &manifest.proofs {
        let at = format!("proof {}", proof.id);
        every.push((format!("{at}.title"), proof.title.as_str()));
        every.push((format!("{at}.request.path"), proof.request.path.as_str()));
        every.push((format!("{at}.why"), proof.why.as_str()));
    }
    for entry in &manifest.contributions {
        let at = format!("contribution {}", entry.id);
        for (field, text) in [
            ("title", entry.title.as_deref()),
            ("why", entry.why.as_deref()),
            ("action", entry.action.as_deref()),
            ("detail", entry.detail.as_deref()),
            (
                "request.path",
                entry.request.as_ref().map(|request| request.path.as_str()),
            ),
        ] {
            if let Some(text) = text {
                every.push((format!("{at}.{field}"), text));
            }
        }
    }
    for recipe in &manifest.recipes {
        let at = format!("recipe {}", recipe.id);
        every.push((format!("{at}.title"), recipe.title.as_str()));
        every.push((format!("{at}.why"), recipe.why.as_str()));
        for step in &recipe.steps {
            if let Some(body) = &step.call.body {
                every.push((format!("{at}.step {}.call.body", step.id), body.as_str()));
            }
        }
    }
    for secret in &manifest.secrets {
        every.push((format!("secret {}.why", secret.id), secret.why.as_str()));
    }
    for over in &manifest.overrides {
        every.push((format!("override {}.why", over.id), over.why.as_str()));
    }
    every
}

#[cfg(test)]
pub(crate) mod tests;
