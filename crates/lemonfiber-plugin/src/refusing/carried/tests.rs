// Each value a manifest carries into a file something else reads, put in front of
// the reader with that file's own punctuation in it. What is asserted is the
// refusal and where it is placed: a manifest carrying any of these is one this
// build does not act on, so nothing below reaches a writer.
use super::{is_directory, is_label, is_reference, is_route, substituted};
use crate::refusing::tests::{names, without};

/// A manifest this build would act on, with its one service renamed.
fn renamed(id: &str) -> Vec<String> {
    without(
        "[[service]]\nid          = \"komga\"",
        &format!("[[service]]\nid          = {}", toml(id)),
    )
}

/// A string as TOML reads it back, escapes included.
fn toml(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_default()
}

#[test]
fn a_line_break_in_a_configuration_directory_is_refused() {
    let said = without(
        "config_path = \"/config\"",
        &format!("config_path = {}", toml("/config\n      - /:/host")),
    );
    assert!(
        names(&said, &["komga.config_path", "plain absolute directory"]),
        "got: {said:?}"
    );
}

#[test]
fn a_configuration_directory_outside_its_alphabet_is_refused() {
    for path in ["/config dir", "/config:ro", "/config,z", "/c'o'nfig"] {
        let said = without(
            "config_path = \"/config\"",
            &format!("config_path = {}", toml(path)),
        );
        assert!(
            names(&said, &["komga.config_path", "`._/-`"]),
            "{path}: {said:?}"
        );
    }
}

#[test]
fn a_line_break_in_an_image_is_refused() {
    let said = without(
        "image       = \"docker.io/gotson/komga\"",
        &format!(
            "image       = {}",
            toml("docker.io/gotson/komga\n    privileged: true\n    x: a/b")
        ),
    );
    assert!(
        names(&said, &["komga.image", "not a registry path"]),
        "got: {said:?}"
    );
}

/// A comment would end the value before the digest is joined to it, so what ran
/// would be the registry's newest rather than what was pinned.
#[test]
fn an_image_that_would_end_before_its_digest_is_refused() {
    let said = without(
        "image       = \"docker.io/gotson/komga\"",
        "image       = \"docker.io/gotson/komga #/b\"",
    );
    assert!(
        names(&said, &["komga.image", "not a registry path"]),
        "got: {said:?}"
    );
}

#[test]
fn an_image_the_stack_would_substitute_into_is_refused() {
    let said = without(
        "image       = \"docker.io/gotson/komga\"",
        "image       = \"registry.example/${SECRET}/komga\"",
    );
    assert!(
        names(&said, &["komga.image", "not a registry path"]),
        "got: {said:?}"
    );
}

/// A tag is refused by its own rule, and not a second time for its grammar.
#[test]
fn an_image_carrying_a_tag_is_refused_once() {
    let said = without(
        "image       = \"docker.io/gotson/komga\"",
        "image       = \"docker.io/gotson/komga:latest\"",
    );
    assert_eq!(
        said.iter()
            .filter(|one| one.contains("komga.image"))
            .count(),
        1,
        "got: {said:?}"
    );
}

/// The id is a directory name and a container name, so a parent link, an absolute
/// path and a separator are each refused.
#[test]
fn a_service_id_that_is_not_one_label_is_refused() {
    for id in [
        "..",
        "/etc",
        "a/b",
        "Komga",
        "komga_sync",
        "-komga",
        "komga\n",
    ] {
        let said = renamed(id);
        assert!(names(&said, &[".id", "one DNS label"]), "{id:?}: {said:?}");
    }
}

#[test]
fn a_hostname_that_is_not_one_label_is_refused() {
    for hostname in [
        "comics.example",
        "comics {\n\treverse_proxy elsewhere:1\n}\nmore",
        "Comics",
        "comics:8080",
        "",
    ] {
        let said = without(
            "hostname        = \"comics\"",
            &format!("hostname        = {}", toml(hostname)),
        );
        assert!(
            names(&said, &["wiring #1.hostname", "one DNS label"]),
            "{hostname:?}: {said:?}"
        );
    }
}

/// Every piece of prose the dashboard is given, with each thing it would expand.
#[test]
fn dashboard_prose_the_dashboard_would_expand_is_refused() {
    let cases = [
        (
            "description = \"Reads your comics on any browser\"",
            "description = \"Reads {{HOMEPAGE_VAR_KEY}}\"",
            "plugin.description",
        ),
        (
            "name        = \"Komga\"\nimage",
            "name        = \"Komga ${KEY}\"\nimage",
            "service komga.name",
        ),
        (
            "dashboard_group = \"Library\"",
            "dashboard_group = \"{{HOMEPAGE_FILE_KEY}}\"",
            "wiring #1.dashboard_group",
        ),
    ];
    for (before, after, at) in cases {
        let said = without(before, after);
        assert!(names(&said, &[at, "never expanded"]), "{at}: {said:?}");
    }
}

/// Each place a manifest declares a request, with a path that is not a route.
#[test]
fn a_request_path_that_is_not_a_route_is_refused_wherever_it_is_declared() {
    let cases = [
        (
            "path = \"/actuator/health\" }\nexpect  = { status = 200, json",
            "path = \"@elsewhere:9000/x\" }\nexpect  = { status = 200, json",
            "proof komga.serves.request.path",
        ),
        (
            "path = \"/api/v1/claim\" }",
            "path = \"/api/v1/claim#x\" }",
            "contribution komga:claimed.request.path",
        ),
        (
            "id      = \"guarded\"\nrequest = { method = \"GET\", path = \"/api/v1/series\" }",
            "id      = \"guarded\"\nrequest = { method = \"GET\", path = \"api\\\\x\" }",
            "claim media.serve probe guarded.request.path",
        ),
    ];
    for (before, after, at) in cases {
        let said = without(before, after);
        assert!(names(&said, &[at, "not a route"]), "{at}: {said:?}");
    }
}

/// The fields the sweep for unreadable text used to pass over, each carrying one.
#[test]
fn a_control_character_is_named_in_every_field_that_reaches_a_file() {
    let bell = "\\u0007";
    let cases = [
        (
            "config_path = \"/config\"".to_owned(),
            format!("config_path = \"/config{bell}\""),
            "komga.config_path",
        ),
        (
            "hostname        = \"comics\"".to_owned(),
            format!("hostname        = \"comics{bell}\""),
            "hostname",
        ),
        (
            "path = \"/api/v1/claim\" }".to_owned(),
            format!("path = \"/api/v1/claim{bell}\" }}"),
            "komga:claimed.request.path",
        ),
    ];
    for (before, after, at) in cases {
        let said = without(&before, &after);
        assert!(names(&said, &[at, "U+0007"]), "{at}: {said:?}");
    }
    let said = renamed("komga\u{7}");
    assert!(names(&said, &[".id", "U+0007"]), "got: {said:?}");
}

#[test]
fn a_label_is_lowercase_letters_digits_and_inner_hyphens() {
    for good in ["a", "komga", "komga-sync", "a1", &"a".repeat(63)] {
        assert!(is_label(good), "{good}");
    }
    for bad in ["", "-a", "a-", "A", "a.b", "a_b", "a b", &"a".repeat(64)] {
        assert!(!is_label(bad), "{bad}");
    }
}

#[test]
fn a_reference_is_what_a_registry_reads_as_a_name() {
    for good in [
        "komga",
        "gotson/komga",
        "docker.io/gotson/komga",
        "ghcr.io/a/b-c",
        "localhost/komga",
        "localhost:5000/komga",
        "registry.example.com:443/a__b/c.d",
        "Registry.Example/a",
        "a---b",
        "a_b/c.d/e-f",
    ] {
        assert!(is_reference(good), "{good}");
    }
    for bad in [
        "",
        "Komga",
        "a/",
        "/a",
        "a//b",
        "a/b:tag",
        "a b",
        "-a",
        "a-",
        "a..b",
        "a___b",
        "a_",
        "host:/a",
        "host:/",
        "host:123456/a",
        "-host.io/a",
        "host-.io/a",
        "host..io/a",
        "host.io:a/b",
        "a/B",
        &format!("a/{}", "b".repeat(255)),
    ] {
        assert!(!is_reference(bad), "{bad}");
    }
}

#[test]
fn a_directory_is_written_in_letters_digits_and_four_marks() {
    assert!(is_directory("/app/data-1/x_y.z"));
    for bad in ["/a b", "/a:b", "/a$b", "/a\nb", "/a{b"] {
        assert!(!is_directory(bad), "{bad:?}");
    }
}

#[test]
fn a_route_starts_at_the_root_and_names_nowhere_else() {
    for good in ["/", "/api/v1/series", "/System/Info?format=json"] {
        assert!(is_route(good), "{good}");
    }
    for bad in [
        "", "api", "@h:1/x", "/a@b", "/a\\b", "/a#b", "/a b", "/a\n", "/a\u{7}",
    ] {
        assert!(!is_route(bad), "{bad:?}");
    }
}

#[test]
fn what_is_substituted_is_named() {
    assert_eq!(substituted("costs $5"), Some("$"));
    assert_eq!(substituted("{{KEY}}"), Some("{{"));
    assert_eq!(substituted("a {single} brace"), None);
}
