//! What the image and every NAS template under `packaging/` hold.
//!
//! The templates are not code, and nothing compiles them, so what they promise is
//! held here: the container shares the host's network, so the web surface is served
//! on the host's loopback and the stack's services are reached where they publish;
//! the command each one runs is one this binary parses as the web surface on this
//! machine only, never on the network; the socket is mounted and the container runs
//! as somebody other than root, joined to the socket's group; every directory is
//! mounted at the same path inside as out; and each says what the socket is.
//!
//! The command is parsed by the same parser the binary uses, so a template that
//! asked for the network would be caught by what `--lan` means rather than by its
//! spelling.

use std::fs;
use std::path::Path;

use clap::Parser as _;
use lemonfiber::cli::{Cli, Request};

use crate::source_tree::workspace_root;

/// The templates that are Compose files, one per platform that reads one.
const COMPOSED: &[&str] = &[
    "packaging/truenas/compose.yaml",
    "packaging/synology/compose.yaml",
    "packaging/compose/compose.yaml",
];

/// The Unraid template.
const UNRAID: &str = "packaging/unraid/lemonfiber.xml";

/// The image's build.
const DOCKERFILE: &str = "packaging/image/Dockerfile";

/// The image's documentation.
const README: &str = "packaging/README.md";

/// Where the host's socket is, and where every template mounts it.
const SOCKET: &str = "/var/run/docker.sock";

/// The words every template and the documentation state the socket's power in.
const POWER: &str = "control of the host's Docker";

/// The placeholder the release writes the image's digest into.
const IMAGE: &str = "{{IMAGE}}";

fn read(path: &str) -> String {
    fs::read_to_string(workspace_root().join(path))
        .unwrap_or_else(|_| unreachable!("{path} is in the tree"))
}

/// A Compose value with every `${NAME:-default}` read as its default, and every
/// `${NAME}` or `${NAME:?…}` read as the operator's value, which is not known here.
fn defaulted(value: &str) -> Option<String> {
    let mut out = String::new();
    let mut rest = value;
    while let Some(start) = rest.find("${") {
        out.push_str(rest.get(..start)?);
        let after = rest.get(start + 2..)?;
        let end = after.find('}')?;
        let inner = after.get(..end)?;
        out.push_str(inner.split_once(":-").map(|(_, default)| default)?);
        rest = after.get(end + 1..)?;
    }
    out.push_str(rest);
    Some(out)
}

/// A Compose value with every variable named bare, whatever default or demand it
/// carries, so two spellings of one variable compare as the one path they are.
fn plain(value: &str) -> String {
    let mut out = String::new();
    let mut rest = value;
    while let Some(start) = rest.find("${") {
        let (before, after) = rest.split_at(start);
        out.push_str(before);
        let Some(end) = after.find('}') else {
            break;
        };
        let inner = after.get(2..end).unwrap_or_default();
        let name = inner.split([':', '-', '?']).next();
        out.push_str("${");
        out.push_str(name.unwrap_or_default());
        out.push('}');
        rest = after.get(end + 1..).unwrap_or_default();
    }
    out.push_str(rest);
    out
}

/// What the binary makes of a command, given as the words after the program name.
fn serves_only_this_machine(words: &[String], whose: &str) {
    let argv = std::iter::once("lemonfiber".to_owned()).chain(words.iter().cloned());
    let Ok(cli) = Cli::try_parse_from(argv) else {
        unreachable!("{whose}: the binary does not parse {words:?}");
    };
    let Some(Request::Ui(ui)) = cli.command else {
        unreachable!("{whose}: {words:?} is not the web surface");
    };
    assert!(
        !ui.lan,
        "{whose}: the surface is offered to the network: {words:?}"
    );
    assert!(
        ui.no_browser,
        "{whose}: a container has no desktop to open: {words:?}"
    );
    assert!(
        ui.port.is_some(),
        "{whose}: a port nobody chose changes at every start: {words:?}"
    );
}

/// The `lemonfiber` service of a Compose-shaped template.
fn service(path: &str) -> serde_yaml_ng::Value {
    let Ok(document) = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&read(path)) else {
        unreachable!("{path} is YAML");
    };
    field(&field(&document, "services"), "lemonfiber")
}

/// One key of a YAML mapping, or nothing where it is absent.
fn field(value: &serde_yaml_ng::Value, key: &str) -> serde_yaml_ng::Value {
    value.get(key).cloned().unwrap_or_default()
}

fn strings(value: &serde_yaml_ng::Value) -> Vec<String> {
    value
        .as_sequence()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// Whether a user is root, by name or by number, alone or with a group.
fn is_root(user: &str) -> bool {
    let who = user.split(':').next().unwrap_or_default();
    who == "0" || who == "root"
}

#[test]
fn every_compose_template_shares_the_hosts_network_and_serves_its_loopback() {
    for path in COMPOSED {
        let service = service(path);
        assert_eq!(
            field(&service, "network_mode").as_str(),
            Some("host"),
            "{path}"
        );
        assert_eq!(field(&service, "image").as_str(), Some(IMAGE), "{path}");
        let words: Vec<String> = strings(&field(&service, "command"))
            .iter()
            .map(|word| defaulted(word).unwrap_or_else(|| word.clone()))
            .collect();
        serves_only_this_machine(&words, path);
    }
}

#[test]
fn every_compose_template_mounts_the_socket_and_runs_as_its_group_rather_than_root() {
    for path in COMPOSED {
        let service = service(path);
        let volumes: Vec<String> = strings(&field(&service, "volumes"))
            .iter()
            .map(|volume| plain(volume))
            .collect();
        assert!(
            volumes.contains(&format!("{SOCKET}:{SOCKET}")),
            "{path}: {volumes:?}"
        );
        let named = field(&service, "user");
        let user = named.as_str().unwrap_or("root");
        assert!(!is_root(user), "{path} runs as {user}");
        assert!(
            !strings(&field(&service, "group_add")).is_empty(),
            "{path} does not join the socket's group"
        );
    }
}

/// Every directory a template mounts is mounted at its own path, and lemonfiber's own
/// files are kept beneath one of them, so the stack it writes is somewhere the host
/// has at the same path.
#[test]
fn every_compose_template_mounts_each_directory_at_the_same_path_inside_and_out() {
    for path in COMPOSED {
        let service = service(path);
        let volumes: Vec<String> = strings(&field(&service, "volumes"))
            .iter()
            .map(|volume| plain(volume))
            .collect();
        let mut inside = Vec::new();
        for volume in &volumes {
            let Some((host, container)) = volume.split_once(':') else {
                unreachable!("{path}: {volume} is not host:container");
            };
            assert_eq!(host, container, "{path}: {volume}");
            inside.push(container.to_owned());
        }
        for key in ["XDG_CONFIG_HOME", "XDG_DATA_HOME"] {
            let Some(at) = field(&field(&service, "environment"), key)
                .as_str()
                .map(plain)
            else {
                unreachable!("{path} does not say where lemonfiber keeps its files ({key})");
            };
            assert!(
                inside
                    .iter()
                    .any(|mounted| mounted != SOCKET && Path::new(&at).starts_with(mounted)),
                "{path}: {key}={at} is under none of {inside:?}"
            );
        }
    }
}

/// The value of one element of the Unraid template.
fn element<'a>(text: &'a str, name: &str) -> &'a str {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    text.split_once(&open)
        .and_then(|(_, after)| after.split_once(&close))
        .map_or("", |(value, _)| value)
}

#[test]
fn the_unraid_template_holds_what_the_compose_ones_do() {
    let text = read(UNRAID);
    assert_eq!(element(&text, "Network"), "host");
    assert_eq!(element(&text, "Repository"), IMAGE);
    let words: Vec<String> = element(&text, "PostArgs")
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    serves_only_this_machine(&words, UNRAID);

    let extra: Vec<&str> = element(&text, "ExtraParams").split_whitespace().collect();
    let user = extra
        .windows(2)
        .find(|pair| pair.first() == Some(&"--user"))
        .and_then(|pair| pair.get(1))
        .copied()
        .unwrap_or("root");
    assert!(!is_root(user), "{UNRAID} runs as {user}");
    assert!(extra.contains(&"--group-add"), "{UNRAID}: {extra:?}");

    let paths: Vec<(&str, &str)> = text
        .lines()
        .filter(|line| line.contains("Type=\"Path\""))
        .map(|line| {
            let target = line
                .split_once("Target=\"")
                .and_then(|(_, after)| after.split_once('"'))
                .map_or("", |(target, _)| target);
            let value = line
                .rsplit_once("\">")
                .and_then(|(_, after)| after.split_once("</Config>"))
                .map_or("", |(value, _)| value);
            (target, value)
        })
        .collect();
    assert!(paths.contains(&(SOCKET, SOCKET)), "{paths:?}");
    for (target, value) in paths {
        assert_eq!(target, value, "{UNRAID} mounts {value} at {target}");
    }
}

/// The image runs the binary and nothing else: every layer is a copy, the base has no
/// shell, and its own command serves this machine only.
#[test]
fn the_image_holds_the_binary_and_no_shell() {
    let text = read(DOCKERFILE);
    let instructions: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    assert!(
        !instructions.iter().any(|line| line.starts_with("RUN")),
        "an instruction that runs something needs a shell to run it in"
    );
    let bases: Vec<&&str> = instructions
        .iter()
        .filter(|line| line.starts_with("FROM"))
        .collect();
    assert!(
        bases
            .last()
            .is_some_and(|base| base.contains("gcr.io/distroless/static")),
        "the image is built on a base with no shell: {bases:?}"
    );
    assert!(
        bases.iter().all(|base| base.contains("@sha256:")),
        "every base is pinned by digest: {bases:?}"
    );
    assert!(instructions.contains(&r#"ENTRYPOINT ["/usr/local/bin/lemonfiber"]"#));
}

#[test]
fn every_template_and_the_documentation_say_what_the_socket_is() {
    for path in COMPOSED.iter().chain([&UNRAID, &README, &DOCKERFILE]) {
        // Read as prose: comment markers and line breaks are where a sentence was
        // wrapped, not part of what it says.
        let said = read(path)
            .lines()
            .map(|line| line.trim().trim_start_matches('#').trim())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            said.contains(POWER),
            "{path} does not say the socket is {POWER}"
        );
    }
}
