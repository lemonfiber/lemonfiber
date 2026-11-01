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

mod asking;
mod bundled;
pub mod carried;
mod choosing;
mod colliding;
mod earlier;
mod evidence;
mod failing;
mod fronting;
mod granting;
mod installing;
mod leaving;
mod minting;
mod naming;
pub(crate) mod program;
mod reaching;
mod readable;
mod recipes;
mod replacing;
mod shaping;
mod speaking;

pub use installing::uninstallable;
pub use recipes::{names_a_header_by_substitution, names_a_path_not_plain, outside};

use std::collections::BTreeSet;

use lemonfiber_manifest::container::MEDIA;

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
    minting::minted(manifest, &mut found);
    minting::stood_in(manifest, &mut found);
    minting::lapsing(manifest, &mut found);
    granting::aimed(manifest, &mut found);
    naming::wired(manifest, &mut found);
    asking::asked(manifest, &mut found);
    naming::about(manifest, &mut found);
    carried::carried(manifest, &mut found);
    evidence::asking(manifest, &mut found);
    evidence::varying(manifest, &mut found);
    evidence::looking(manifest, &mut found);
    program::asked(manifest, &mut found);
    failing::declared(manifest, &mut found);
    requiring(manifest, &mut found);
    recipes::declared(manifest, &mut found);
    readable::readable(manifest, &mut found);
    earlier::named(manifest, &mut found);
    choosing::chosen(manifest, &mut found);
    found
}

/// The name the household reaches the shipped stack's service `service` at, where a part
/// the bundle pins declares one for it.
#[must_use]
pub fn shipped_label(service: &str) -> Option<&'static str> {
    bundled::label_of(service)
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
        filing(service, found);
        contained(manifest, service, found);
        speaking::spoken(service, &manifest.services, found);
        shaping::shaped(service, &manifest.services, found);
        shaping::critical(service, found);
        leaving::stated(service, found);
        fronting::native(service, &manifest.services, found);
        fronting::outside(service, found);
        replacing::replaced(service, found);
        minting::handed(manifest, service, found);
    }
}

/// Each media type the service files, held to the stack manifest's vocabulary: the
/// core builds a folder beneath the data root's media directory from each, so a name
/// outside it could reach a folder no service of the stack files into.
fn filing(service: &Service, found: &mut Vec<Violation>) {
    found.extend(
        service
            .media_types
            .iter()
            .filter(|media_type| !MEDIA.contains(&media_type.as_str()))
            .map(|media_type| Violation {
                location: format!("service {}.media_types", service.id),
                message: format!(
                    "names {media_type:?}, which is not one of {}",
                    MEDIA.join(", ")
                ),
            }),
    );
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
                 runs is declared once, in digest, so a second pin here could disagree with it",
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

/// Every rule the service's container fields break, as the one model holds every
/// service's.
fn contained(manifest: &Manifest, service: &Service, found: &mut Vec<Violation>) {
    found.extend(
        service
            .container(&manifest.plugin.id, &manifest.secrets)
            .broken()
            .into_iter()
            .map(|(location, message)| Violation { location, message }),
    );
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

#[cfg(test)]
pub(crate) mod tests;
