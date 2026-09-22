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
mod colliding;
mod reaching;
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
    use super::{declared, refusals, Violation};
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
    pub(crate) fn said(text: &str) -> Vec<String> {
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
    pub(crate) fn names(said: &[String], words: &[&str]) -> bool {
        said.iter()
            .any(|one| words.iter().all(|word| one.contains(word)))
    }

    /// One edit to a manifest this build would act on, and what it then says about it.
    pub(crate) fn without(before: &str, after: &str) -> Vec<String> {
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
        let said = said(&text);
        assert!(names(&said, &["service", "one service"]), "got: {said:?}");
    }

    #[test]
    fn a_plugin_joining_no_form_is_refused() {
        let said = without(r#"forms       = ["library"]"#, "forms       = []");
        assert!(names(&said, &["plugin.forms"]), "got: {said:?}");
    }

    #[test]
    fn a_digest_with_no_readable_name_beside_it_is_refused() {
        let said = without(r#"tag         = "1.11.0""#, r#"tag         = """#);
        assert!(names(&said, &["service komga.tag"]), "got: {said:?}");
    }

    /// A file the schema refuses never reaches the rules over values.
    #[test]
    fn a_manifest_that_is_not_one_is_answered_by_the_reader_rather_than_here() {
        let said = said("schema_version = 1\n[plugin]\nid = \"komga\"\n");
        assert!(names(&said, &["does not conform"]), "got: {said:?}");
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

    /// A recipe that takes a value out of an answer and says nothing about holding
    /// it, for the two tests that need a reach the reader parses rather than one it
    /// refuses as a shape.
    const UNDECLARED: &str = r#"
[[recipe]]
id    = "adopt-existing-library"
title = "Point it at the comics the stack already files"
why   = "The stack already files comics, and a fresh Komga knows nothing about it."

[[recipe.step]]
id      = "sign-in"
call    = { method = "POST", to = "komga", path = "/api/v1/login" }
expect  = { status = 200 }
capture = [{ name = "token", from = "json.token", origin = "stack-service" }]
"#;

    /// Every way this format lets a manifest reach past what it declares.
    ///
    /// The register itself, as behaviour. Each of these is somewhere a manifest can
    /// name something that is not its own, and each is refused — which is the whole
    /// of what makes reading one worth doing. Kept together rather than left to the
    /// module that implements each, because the thing worth knowing is that the
    /// *list* is answered for: a way in that nobody refused is not visible from
    /// inside the rule that does not cover it.
    #[test]
    fn every_way_a_manifest_can_reach_past_what_it_declares_is_refused() {
        let reaches: [(&str, String, &str); 8] = [
            (
                "a field the format has no declaration for",
                INSTALLABLE.replace(
                    "takes_data  = true",
                    "takes_data  = true\nprivileged = true",
                ),
                "privileged",
            ),
            (
                "an id the stack already holds",
                INSTALLABLE.replace(
                    "[[service]]\nid          = \"komga\"",
                    "[[service]]\nid          = \"jellyfin\"",
                ),
                "service jellyfin.id",
            ),
            (
                "a port the stack already publishes on",
                INSTALLABLE.replace("port        = 25600", "port        = 8096"),
                "service komga.port",
            ),
            (
                "a name the stack's proxy already answers on",
                INSTALLABLE.replace(
                    r#"hostname        = "comics""#,
                    r#"hostname        = "watch""#,
                ),
                "wiring.hostname",
            ),
            (
                "a directory outside the one it is given",
                INSTALLABLE.replace(
                    r#"config_path = "/config""#,
                    r#"config_path = "/data/komga""#,
                ),
                "service komga.config_path",
            ),
            (
                "a capability of lemonfiber's this build does not offer",
                INSTALLABLE.replace(
                    r#"capabilities = ["doctor.contribute"]"#,
                    r#"capabilities = ["doctor.contribute", "service.add"]"#,
                ),
                "service.add",
            ),
            (
                "an identity a register lemonfiber runs already holds",
                INSTALLABLE.replace(
                    r#"id        = "komga:claimed""#,
                    r#"id        = "storage.hardlinks""#,
                ),
                "storage.hardlinks",
            ),
            (
                "a value taken out of an answer and never declared",
                format!("{INSTALLABLE}{UNDECLARED}"),
                "[[secret]]",
            ),
        ];
        for (reach, text, named) in reaches {
            let said = said(&text);
            assert!(
                names(&said, &[named]),
                "{reach} is refused, and named: {said:?}"
            );
        }
    }

    /// Every way a plugin could ask for more of the machine, and what it is called.
    ///
    /// Each is spelled the way a container engine spells it, because that is what an
    /// author copying an entry out of somewhere else would write. `grants` earns a row
    /// beside `cap_add` rather than being folded into it: it is what the *stack's* own
    /// manifest calls a kernel capability, so it is the spelling somebody reading
    /// `stack.toml` for an example would reach for.
    const MORE_OF_THE_MACHINE: [(&str, &str, &str); 11] = [
        (
            "a mount of its own",
            r#"volumes = ["/etc:/etc"]"#,
            "volumes",
        ),
        ("a device", r#"devices = ["/dev/net/tun"]"#, "devices"),
        (
            "a kernel capability",
            r#"cap_add = ["NET_ADMIN"]"#,
            "cap_add",
        ),
        (
            "a kernel capability, as the stack spells it",
            r#"grants = ["NET_ADMIN"]"#,
            "grants",
        ),
        (
            "a network of its own",
            r#"network_mode = "host""#,
            "network_mode",
        ),
        ("a privileged container", "privileged = true", "privileged"),
        ("a user override", r#"user = "0:0""#, "user"),
        ("an entrypoint", r#"entrypoint = "/bin/sh""#, "entrypoint"),
        ("a command", r#"command = "cat /etc/shadow""#, "command"),
        (
            "an environment variable",
            r#"environment = { TZ = "UTC" }"#,
            "environment",
        ),
        (
            "the container runtime's own socket, as a mount by another name",
            r#"volumes_from = ["docker"]"#,
            "volumes_from",
        ),
    ];

    /// The installable manifest with one of those declared on its service.
    fn asking(declared: &str) -> String {
        INSTALLABLE.replace(
            "takes_data  = true",
            &format!("takes_data  = true\n{declared}"),
        )
    }

    /// Every one of them is refused, and the refusal names the field.
    ///
    /// The register above is about reaching past what a manifest *declared*. This is
    /// about reaching past what any manifest may declare at all, which fails
    /// differently: there is no field to ask in, so what comes back is a malformed
    /// manifest rather than a permission withheld. The difference is worth keeping
    /// visible — a rule somebody has to remember and apply can be forgotten for one
    /// plugin, and an absent field cannot.
    #[test]
    fn every_way_a_plugin_could_ask_for_more_of_the_machine_is_refused_by_name() {
        for (reach, declared, named) in MORE_OF_THE_MACHINE {
            let said = said(&asking(declared));
            assert!(
                names(&said, &[named]),
                "{reach} is refused, and named: {said:?}"
            );
        }
    }

    /// And none of them is a manifest with the ask quietly taken out.
    ///
    /// The shape faults yield no manifest at all, so there is nothing for a caller to
    /// install on narrowed terms. Asked of the same list rather than of a few of it,
    /// because *refused* and *narrowed* look identical from a test that only reads
    /// the words in a refusal — and a field that parsed and was dropped would pass
    /// the one above while leaving the plugin running under a declaration nobody
    /// could read.
    #[test]
    fn a_manifest_asking_for_more_of_the_machine_yields_nothing_to_act_on() {
        for (reach, declared, _) in MORE_OF_THE_MACHINE {
            assert!(
                Manifest::from_toml(&asking(declared)).ok().is_none(),
                "{reach} yields no manifest"
            );
        }
    }

    /// An over-reaching manifest is answered with a refusal, not with a smaller
    /// manifest.
    ///
    /// The distinction the whole extension design turns on. Confining one — taking
    /// the reach out and installing the rest — would leave a plugin running under a
    /// declaration that no longer describes it, and the declaration is the only
    /// thing anybody read. So the shape faults yield no manifest at all, and the
    /// value faults leave the manifest exactly as it was written and say why it is
    /// refused.
    #[test]
    fn a_manifest_that_over_reaches_is_refused_rather_than_narrowed() {
        let asking = INSTALLABLE.replace(
            "takes_data  = true",
            "takes_data  = true\nprivileged = true",
        );
        assert!(Manifest::from_toml(&asking).ok().is_none());

        let reaching = format!("{INSTALLABLE}{UNDECLARED}");
        let read = Manifest::from_toml(&reaching).ok();
        let found: Vec<String> = read
            .as_ref()
            .map(|manifest| refusals(manifest, OCCUPIED))
            .unwrap_or_default()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert!(
            names(&found, &["[[secret]]"]),
            "the reach is one the reader parses and the rules refuse by name: {found:?}"
        );
        assert_eq!(
            read.as_ref()
                .and_then(|manifest| manifest.recipes.first())
                .and_then(|recipe| recipe.steps.first())
                .and_then(|step| step.capture.first())
                .map(|capture| capture.name.as_str()),
            Some("token"),
            "and the reach is still there to read, rather than taken out and the rest kept"
        );
    }

    /// There is no field by which a refusal could be downgraded to a warning.
    ///
    /// The other way confinement arrives: not by editing the manifest but by
    /// grading the answer, so that some refusals stop the install and others are
    /// printed. A refusal here is where it is and what is wrong with it, and
    /// nothing else — every one of them is a reason this build will not act on the
    /// file.
    #[test]
    fn a_refusal_carries_nowhere_to_say_it_is_only_advisory() {
        let one = Violation {
            location: "service komga.port".to_owned(),
            message: "is the port the stack already publishes jellyfin on".to_owned(),
        };
        let carried: Vec<String> = serde_json::to_value(&one)
            .ok()
            .and_then(|held| {
                held.as_object()
                    .map(|fields| fields.keys().cloned().collect())
            })
            .unwrap_or_default();
        assert_eq!(
            carried,
            vec!["location".to_owned(), "message".to_owned()],
            "a refusal says where and what, and carries no grade by which one could be \
             reported and proceeded past"
        );
    }
}
