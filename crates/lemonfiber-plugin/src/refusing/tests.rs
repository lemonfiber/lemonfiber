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

[[wiring]]
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

/// A service that names no configuration directory is refused nothing about one.
///
/// The field is optional and leaving it out is what an image reading the convention
/// writes, so there is nothing to hold the rules about where a directory may land
/// to — and a refusal here would be this build refusing the default it publishes.
/// Every fixture these rules have ever been read against declared it.
#[test]
fn a_service_that_names_no_configuration_directory_is_refused_nothing_about_one() {
    let said = without("config_path = \"/config\"\n", "");
    assert_eq!(said, Vec::<String>::new());
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

/// The second service beside the first, which the format now permits.
///
/// `loopback` and `enhancing` where the first is `lan` and `important`, because
/// those two differences are the whole argument for letting a plugin declare two.
/// It declares the first service's own namespaced capability as well, which is the
/// case a core name is refused for and this one is not.
pub(crate) const BESIDE: &str = r#"
[[service]]
id          = "komga-stats"
name        = "Komga statistics"
image       = "docker.io/gotson/komga-stats"
digest      = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
tag         = "0.4.0"
port        = 8181
bind        = "loopback"
criticality = "enhancing"
provides    = ["komga:kobo-sync"]
config_path = "/stats"
"#;

/// A manifest this build would act on, declaring two services.
///
/// Built by editing the single-service one rather than written out, so the two
/// cannot drift: every rule the first service is held to is the same rule, and what
/// this fixture adds is only the three things a second service makes answerable.
pub(crate) fn paired() -> String {
    format!(
        "{}{BESIDE}",
        INSTALLABLE
            .replace("[[wiring]]\n", "[[wiring]]\nservice         = \"komga\"\n")
            .replace(
                "id      = \"komga.serves\"\n",
                "id      = \"komga.serves\"\nservice = \"komga\"\n"
            )
    )
}

/// Two services in one plugin, which is one install and two containers.
#[test]
fn a_plugin_may_declare_a_second_service_beside_the_first() {
    let said = said(&paired());
    assert!(said.is_empty(), "got: {said:?}");
}

/// And none at all is still refused, because the rest of the file is about one.
#[test]
fn a_plugin_declaring_no_service_is_refused() {
    let text = INSTALLABLE
        .split("[[service]]")
        .next()
        .unwrap_or_default()
        .to_owned()
        + "[requires]\ncapabilities = []\n";
    let said = said(&text);
    assert!(
        names(&said, &["service", "declares no service"]),
        "got: {said:?}"
    );
}

/// One id for two containers, which every later rule would resolve to the first.
#[test]
fn two_services_sharing_an_id_are_refused_by_that_id() {
    let said = said(&paired().replace("id          = \"komga-stats\"", "id          = \"komga\""));
    assert!(
        names(&said, &["service komga.id", "declared twice"]),
        "got: {said:?}"
    );
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
