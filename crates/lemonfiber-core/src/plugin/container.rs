//! The container lemonfiber writes for a plugin's service, and the whole of what
//! that service may reach of the machine.
//!
//! A plugin supplies no container definition, and lemonfiber writes one from what it
//! declared. The two halves are one rule rather than two: a format that refused a
//! supplied fragment and then let a declaration widen the generated one would be
//! refusing the paperwork rather than the reach. So the set of fields a manifest may
//! carry *is* the set of things a plugin may ask for, and this is the only place that
//! set is turned into a container.
//!
//! **Everything here is derived and nothing here is remembered.** The profile is the
//! plugin's id with a word in front of it; the interface behind a published port is
//! the tier rendered by lemonfiber's own rule; the source of the configuration mount
//! is lemonfiber's own directory named for the service. None of those is in the
//! install record, because a copy of a derivation is free to disagree with the
//! derivation — so the record holds what was decided and this holds what follows
//! from it.
//!
//! **The mount set is fixed, and that is the whole of what makes the question
//! answerable.** A service gets its own configuration directory, and the library
//! where it declared it needs one. There is no third, and no field anywhere in the
//! format by which a third could be asked for — which is why "what can this plugin
//! reach" is answered by reading the format rather than by inspecting the machine it
//! was installed on.
//!
//! **What refuses a manifest is the reader, once.** This writes the entry the record
//! describes rather than forming a second opinion about it: the values were held to
//! the format when the install was settled, and a rule applied again here would refuse
//! a plugin at a point where nobody is looking at a manifest and nothing could say
//! which line of it was wrong. A record this build would not have written is the
//! register's problem, and it refuses one it cannot read rather than reading it as
//! nothing.
//!
//! **It extends `defaults` rather than `rootless`, and that is a decision rather
//! than an omission.** The bundled stack picks between the two per image, because
//! the pair a `rootless` service is told is read by some images and silently ignored
//! by others — and setting them on an image that ignores them is a no-op that reads
//! like a security control. A plugin has no field by which to say which shape its
//! image is, so writing `rootless` for it would be this build asserting something
//! about a stranger's image that nobody checked.

use serde::Serialize;

use super::installed::{Installed, Placed, Reached};

/// The shared template every bundled service extends, and the service inside it.
///
/// Named as a pair because the two are one reference: `extends` without the file is
/// a service in the same document, and the template deliberately lives in a file no
/// include list names so that it can never become a container of its own.
const TEMPLATE: (&str, &str) = ("compose/_common.yml", "defaults");

/// The word in front of a plugin's id, which is the profile its services sit in.
///
/// Its own profile rather than one the stack already has: a profile is the unit that
/// starts and stops together, and a stranger's service joining `tv` would mean
/// `lemonfiber up tv` could no longer be described without naming what is installed.
const PROFILE: &str = "plugin-";

/// The Compose profile this plugin's services sit in.
///
/// Published because an install has to start exactly them and a removal has to take
/// exactly them back, and a caller that spelled the name itself would be a second
/// answer to which profile the entry above was written into.
#[must_use]
pub fn profile(plugin: &str) -> String {
    format!("{PROFILE}{plugin}")
}

/// Where lemonfiber keeps a service's own configuration directory.
///
/// The source is lemonfiber's and the target is the plugin's: an image that reads
/// its configuration somewhere other than the convention says where, and gets the
/// same one directory mounted where it actually reads it.
const CONFIGURATION: &str = "./config";

/// The library, mounted exactly as every bundled service mounts it.
const LIBRARY: &str = "${DATA_ROOT:-./data}:/data";

/// The interface a household port is published on.
///
/// The same variable and the same fallback the bundled stack's own household
/// services are written with. A second spelling here would be a plugin's service
/// published on terms a bundled one is not, which is the thing the tier exists to
/// prevent.
const HOUSEHOLD: &str = "${LAN_BIND:-0.0.0.0}";

/// The interface an operator surface is published on.
const OPERATOR: &str = "127.0.0.1";

/// The container entries lemonfiber writes for an installed plugin.
///
/// One document rather than one entry, because a plugin is several services in the
/// contract even where this generation of the format admits one — and a caller
/// handed a single entry would be a caller that has to learn to join them the day
/// the second arrives.
///
/// A function of the record alone. Everything it needs was settled when the plugin
/// was installed, so this answers the same on a machine whose plugin source is long
/// gone as on the one that installed it.
///
/// **Serialised, never formatted.** The document is built as values and handed to a
/// YAML writer, so a value holds its place whatever it carries: a line break is
/// escaped inside its scalar rather than starting a key of its own. The reader has
/// already refused every value that would need that, and this is the second wall
/// rather than the first — a record is read back from disk, and what is on disk is
/// not always what this build wrote.
#[must_use]
pub fn written(installed: &Installed) -> String {
    let document = Document {
        services: Services(
            installed
                .services
                .iter()
                .map(|placed| (placed.service.as_str(), entry(&installed.plugin, placed)))
                .collect(),
        ),
    };
    // Maps of strings and lists of strings, which a YAML writer cannot fail to write.
    serde_yaml_ng::to_string(&document).unwrap_or_default()
}

/// The document an overlay is: services, and nothing else at the top.
#[derive(Serialize)]
struct Document<'a> {
    /// Every one of the plugin's services, by name.
    services: Services<'a>,
}

/// Services in the order the record keeps them, so the file reads the same twice.
struct Services<'a>(Vec<(&'a str, Entry)>);

impl Serialize for Services<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.0.iter().map(|(name, entry)| (name, entry)))
    }
}

/// One service's entry, and the whole of what one can carry.
///
/// A key is a field here, so a key a plugin may not have — a mount of its own, a
/// device, a kernel grant, a network mode, a user, an entrypoint, a command, an
/// environment — is one there is no field for.
#[derive(Serialize)]
struct Entry {
    /// The template it extends.
    extends: Extends,
    /// The registry path joined to the digest that pins it.
    image: String,
    /// The plugin's own profile.
    profiles: Vec<String>,
    /// Where it is published, where it listens.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    ports: Vec<String>,
    /// The library where it asked for it, and its own configuration directory.
    volumes: Vec<String>,
}

/// The template reference an entry extends.
#[derive(Serialize)]
struct Extends {
    /// The file the template is in.
    file: &'static str,
    /// The service inside it.
    service: &'static str,
}

/// One service's entry, in the shape the bundled fragments are written in.
///
/// The digest rather than the tag, joined to the image by the `@` a registry reads:
/// the tag is a name its publisher can repoint, so an entry written from one could
/// run something other than what was reviewed with nothing in the manifest having
/// changed.
///
/// Every value the plugin supplied is written as a literal: Compose substitutes
/// `${…}` from the stack's environment file in any value, quoted or not, and `$$` is
/// how a Compose file says a dollar that is only a dollar. The two variables this
/// build writes itself — the interface and the library — are the only substitutions
/// an entry carries.
fn entry(plugin: &str, placed: &Placed) -> Entry {
    let (file, service) = TEMPLATE;
    let ports = placed
        .reached
        .as_ref()
        .map(|reached| {
            let port = reached.port();
            format!("{}:{port}:{port}", published(reached))
        })
        .into_iter()
        .collect();
    let mut volumes = Vec::new();
    if placed.takes_data {
        volumes.push(LIBRARY.to_owned());
    }
    volumes.push(format!(
        "{CONFIGURATION}/{}:{}",
        literal(&placed.service),
        literal(&placed.config_path)
    ));
    Entry {
        extends: Extends { file, service },
        image: format!("{}@{}", literal(&placed.image), literal(&placed.digest)),
        profiles: vec![profile(plugin)],
        ports,
        volumes,
    }
}

/// A value Compose reads back as exactly itself, substitution included.
fn literal(text: &str) -> String {
    text.replace('$', "$$")
}

/// Which interface a tier publishes on.
///
/// Read off the tier and never off a field, because there is no field: a plugin that
/// could write its own interface could put a full-control surface on the household
/// network without touching anything the web-security checks inspect.
const fn published(reached: &Reached) -> &'static str {
    match reached {
        Reached::Loopback { .. } => OPERATOR,
        Reached::Household { .. } => HOUSEHOLD,
    }
}

#[cfg(test)]
mod tests;
