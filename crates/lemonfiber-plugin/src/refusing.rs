//! What a manifest is refused for, once it is the shape the schema describes.
//!
//! [`crate::conforming`] answers whether a file is a manifest at all — the fields, the
//! kinds, the closed sets. What is left is everything the shape cannot say: that a
//! digest is a digest, that a path is one directory and not the library, that a
//! capability asked for is one this build has. Those are rules over values, and a
//! generated schema is the wrong place for them: a schema an author's editor enforces
//! has to describe the reader exactly, and a reader that refused a well-shaped digest
//! for being the wrong length would be describing a rule rather than a shape.
//!
//! Every one of them is reported in one pass with the others, because an author fixing
//! a third-party manifest one fault per run is guessing. Nothing here stops at the first
//! thing it finds.
//!
//! **A manifest is refused whole.** Nothing here applies part of one: the shape is
//! answered for before a value is looked at, and the values are answered for together,
//! so what a caller gets back is every reason this build would not act on the file
//! rather than the first one.

mod recipes;

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
/// An empty answer is a manifest this build would act on.
#[must_use]
pub fn refusals(manifest: &Manifest, occupied: &[&str]) -> Vec<Violation> {
    let mut found = claiming::violations(manifest, occupied);
    declaring(&manifest.plugin, &mut found);
    running(manifest, &mut found);
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
    if !plugin
        .id
        .chars()
        .all(|letter| letter.is_ascii_lowercase() || letter.is_ascii_digit() || letter == '-')
        || plugin.id.is_empty()
    {
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
/// One service, because this generation of the format describes one addition to a stack
/// that already exists. Two would make "which one did I install" a question with no good
/// answer, and none would make the rest of the manifest describe nothing.
fn running(manifest: &Manifest, found: &mut Vec<Violation>) {
    if manifest.services.len() != 1 {
        found.push(Violation {
            location: "service".to_owned(),
            message: format!(
                "this generation of the format describes exactly one service and {} are declared",
                manifest.services.len()
            ),
        });
    }
    for service in &manifest.services {
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
    let hex = service.digest.strip_prefix(prefix);
    let good = hex.is_some_and(|after| {
        after.len() == length && after.chars().all(|letter| letter.is_ascii_hexdigit())
    });
    if !good {
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
    if !path.starts_with('/') || path == "/" || path.contains("..") || path.contains('$') {
        found.push(Violation {
            location: at,
            message: format!(
                "{path} is not one plain absolute directory; what is permitted is a single \
                 absolute path that is not the root, with no `..` and nothing interpolated"
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
                "{name} is not something this build offers a plugin; what it offers is: {}",
                if listed.is_empty() {
                    "nothing yet — each arrives with the mechanism that provides it".to_owned()
                } else {
                    listed.clone()
                }
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
        every.push((format!("{at}.name"), service.name.as_str()));
        every.push((format!("{at}.image"), service.image.as_str()));
        every.push((format!("{at}.tag"), service.tag.as_str()));
    }
    for proof in &manifest.proofs {
        let at = format!("proof {}", proof.id);
        every.push((format!("{at}.title"), proof.title.as_str()));
        every.push((format!("{at}.why"), proof.why.as_str()));
    }
    for entry in &manifest.contributions {
        let at = format!("contribution {}", entry.id);
        for (field, text) in [
            ("title", entry.title.as_deref()),
            ("why", entry.why.as_deref()),
            ("action", entry.action.as_deref()),
            ("detail", entry.detail.as_deref()),
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
pub(crate) mod tests {
    use super::{declared, refusals};
    use crate::schema::Manifest;

    /// The identities lemonfiber's own registers hold, which these fixtures avoid.
    pub(crate) const OCCUPIED: &[&str] = &["storage.hardlinks"];

    /// A manifest this build would act on.
    ///
    /// Apart rather than reusing the whole-format fixture, because the two are for
    /// different questions. That one carries every block the contract declares so the
    /// types can be held to it field for field, recipes included — and a recipe is a
    /// thing this build refuses, so a fixture proving *nothing is refused* cannot be the
    /// same file.
    pub(crate) const INSTALLABLE: &str = r#"
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
health      = { kind = "http", path = "/actuator/health", timeout_s = 90 }
criticality = "important"
media_types = ["comics"]
takes_data  = true
provides    = ["media.serve", "komga:kobo-sync"]
config_path = "/config"

[[claim]]
capability = "media.serve"

[[claim.probe]]
id      = "guarded"
request = { method = "GET", path = "/api/v1/series" }
expect  = { status = 401 }
fixture = "fixtures/media-serve-guarded.json"

[[claim.probe]]
id      = "catalogue"
request = { method = "GET", path = "/api/v1/series" }
expect  = { status = 200, json_has_keys = ["content"], json_types = { content = "list" } }
fixture = "fixtures/media-serve-catalogue.json"

[wiring]
hostname        = "comics"
dashboard_group = "Library"

[[proof]]
id      = "komga.serves"
title   = "Komga answers on its declared health path"
request = { method = "GET", path = "/actuator/health" }
expect  = { status = 200, json = { status = "UP" } }
fixture = "fixtures/health.json"
why     = "The path the health probe asks for is one this image serves."

[[contribution]]
at        = "doctor.check"
id        = "komga:claimed"
title     = "Komga has an administrator, so nobody else can become one"
category  = "services"
request   = { method = "GET", path = "/api/v1/claim" }
expect    = { status = 200, json = { isClaimed = true } }
fixture   = "fixtures/api-v1-claim-claimed.json"
timeout_s = 10
service   = "komga"
why       = "An unclaimed Komga hands administrator to whoever asks first."

[[contribution]]
at     = "doctor.remedy"
id     = "komga:claim-it"
for    = "komga:claimed"
action = "Open Komga and create the administrator account"
detail = "POST /api/v1/claim with an email and a password creates it."
why    = "Until somebody does, the first caller on the household network becomes it."

[requires]
capabilities = ["doctor.contribute"]
"#;

    /// What this build says about a manifest, as one line per refusal.
    fn said(text: &str) -> Vec<String> {
        Manifest::from_toml(text).map_or_else(
            |refused| vec![refused.to_string()],
            |manifest| {
                refusals(&manifest, OCCUPIED)
                    .iter()
                    .map(ToString::to_string)
                    .collect()
            },
        )
    }

    /// Whether some refusal carries every one of these words.
    fn names(said: &[String], words: &[&str]) -> bool {
        said.iter()
            .any(|one| words.iter().all(|word| one.contains(word)))
    }

    /// One edit to a manifest this build would act on, and what it then says about it.
    fn without(before: &str, after: &str) -> Vec<String> {
        assert!(
            INSTALLABLE.contains(before),
            "the fixture still says {before:?}"
        );
        said(&INSTALLABLE.replace(before, after))
    }

    #[test]
    fn a_manifest_this_build_would_act_on_is_refused_nothing() {
        assert_eq!(said(INSTALLABLE), Vec::<String>::new());
    }

    /// A manifest the schema refuses yields no manifest at all, not a partial one.
    ///
    /// The whole point of answering for the shape before the values is that nothing
    /// downstream is ever handed the half that parsed. A reader that returned what it
    /// managed would make "never applied on the strength of the parts that did parse" a
    /// rule somebody has to remember rather than one the types keep.
    #[test]
    fn a_manifest_the_schema_refuses_yields_nothing_to_act_on() {
        let text = INSTALLABLE.replace(
            "takes_data  = true",
            "takes_data  = true\nprivileged = true",
        );
        assert!(Manifest::from_toml(&text).is_err());
        assert!(Manifest::from_toml(&text).ok().is_none());
    }

    #[test]
    fn an_image_named_by_tag_alone_is_refused() {
        let said = without(
            r#"digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945""#,
            r#"digest      = "1.11.0""#,
        );
        assert!(
            names(&said, &["service komga.digest", "sha256:"]),
            "got: {said:?}"
        );
    }

    #[test]
    fn a_digest_of_the_wrong_length_is_not_a_digest() {
        let said = without(
            "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
            "sha256:4f53cda",
        );
        assert!(names(&said, &["service komga.digest"]), "got: {said:?}");
    }

    #[test]
    fn an_image_carrying_its_own_tag_is_refused_because_two_pins_can_disagree() {
        let said = without(
            r#"image       = "docker.io/gotson/komga""#,
            r#"image       = "docker.io/gotson/komga:1.11.0""#,
        );
        assert!(names(&said, &["service komga.image"]), "got: {said:?}");
    }

    /// A registry that answers on a port is a host, not a version.
    #[test]
    fn a_registry_host_with_a_port_is_not_read_as_a_tag() {
        let said = without(
            r#"image       = "docker.io/gotson/komga""#,
            r#"image       = "localhost:5000/gotson/komga""#,
        );
        assert!(
            !names(&said, &["service komga.image"]),
            "the host's port is not a pin: {said:?}"
        );
    }

    #[test]
    fn a_port_with_no_tier_behind_it_is_refused() {
        let said = without(r#"bind        = "lan""#, "");
        assert!(names(&said, &["service komga.bind"]), "got: {said:?}");
    }

    #[test]
    fn a_configuration_directory_inside_the_library_is_refused() {
        let said = without(
            r#"config_path = "/config""#,
            r#"config_path = "/data/komga""#,
        );
        assert!(
            names(&said, &["service komga.config_path", "/data"]),
            "got: {said:?}"
        );
    }

    #[test]
    fn a_configuration_directory_that_is_not_one_plain_absolute_path_is_refused() {
        for asked in ["/", "config", "/srv/../etc", "/srv/$HOME"] {
            let said = without(
                r#"config_path = "/config""#,
                &format!("config_path = \"{asked}\""),
            );
            assert!(
                names(&said, &["service komga.config_path"]),
                "{asked} is refused: {said:?}"
            );
        }
    }

    #[test]
    fn a_plugin_with_no_licence_is_refused() {
        let said = without(r#"license     = "MIT""#, r#"license     = """#);
        assert!(names(&said, &["plugin.license"]), "got: {said:?}");
    }

    /// A licence that is merely not open is recorded and shown, never refused.
    #[test]
    fn a_licence_that_is_not_open_is_not_refused() {
        let said = without(r#"license     = "MIT""#, r#"license     = "Plex-EULA""#);
        assert!(!names(&said, &["plugin.license"]), "got: {said:?}");
    }

    #[test]
    fn an_upstream_an_operator_cannot_go_and_look_at_is_refused() {
        let said = without(
            r#"upstream    = "https://github.com/gotson/komga""#,
            r#"upstream    = "ask the author""#,
        );
        assert!(names(&said, &["plugin.upstream"]), "got: {said:?}");
    }

    #[test]
    fn an_id_carrying_a_separator_is_refused_because_it_is_the_namespace() {
        let said = without(
            r#"id          = "komga""#,
            r#"id          = "komga:reader""#,
        );
        assert!(names(&said, &["plugin.id"]), "got: {said:?}");
    }

    #[test]
    fn a_capability_this_build_does_not_offer_is_refused_by_name() {
        let said = without(
            r#"capabilities = ["doctor.contribute"]"#,
            r#"capabilities = ["doctor.contribute", "service.add"]"#,
        );
        assert!(
            names(&said, &["requires.capabilities", "service.add"]),
            "got: {said:?}"
        );
        assert!(
            !said.iter().any(|one| one.contains("version")),
            "and never by naming a version: {said:?}"
        );
    }

    #[test]
    fn a_second_service_is_refused_because_the_format_describes_one_addition() {
        let text = format!(
            "{INSTALLABLE}\n[[service]]\nid = \"other\"\nname = \"Other\"\n\
             image = \"docker.io/x/y\"\n\
             digest = \"sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945\"\n\
             tag = \"1\"\ncriticality = \"optional\"\n"
        );
        assert!(
            names(&said(&text), &["service", "one service"]),
            "got: {:?}",
            said(&text)
        );
    }

    #[test]
    fn a_value_a_diff_cannot_show_is_refused() {
        let said = without(
            r#"description = "Reads your comics on any browser""#,
            "description = \"Reads your \\u0007comics\"",
        );
        assert!(
            names(&said, &["plugin.description", "U+0007"]),
            "got: {said:?}"
        );
    }

    /// Every string the manifest declares is one this sweep looks at.
    ///
    /// The sweep is a list, and a list is a thing that goes stale: a field added to the
    /// format and not added here is one an unreadable value can be written into, and
    /// nothing else would notice. Held to the count the whole fixture produces, so
    /// adding a field without adding it here is a failure rather than a silence.
    #[test]
    fn every_string_the_whole_manifest_declares_is_swept() {
        let read = Manifest::from_toml(crate::schema::tests::WHOLE).ok();
        let swept = read.as_ref().map(|manifest| declared(manifest).len());
        assert_eq!(swept, Some(22));
    }
}
