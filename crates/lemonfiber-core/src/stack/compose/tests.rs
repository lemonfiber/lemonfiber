use std::path::{Path, PathBuf};

use lemonfiber_manifest::Manifest;

use super::{build, Action, Plan, Settings};
use crate::config::Protocols;
use crate::platform::Environment;
use crate::stack::closure::resolve;

const STACK: &str = include_str!("../../../../../assets/media-stack/stack.toml");

fn stack_dir() -> &'static Path {
    Path::new("/opt/lemonfiber/stack")
}

fn plan(forms: &[&str]) -> Option<Plan> {
    let named: Vec<String> = forms.iter().map(|form| (*form).to_owned()).collect();
    Manifest::from_toml(STACK)
        .ok()
        .and_then(|manifest| resolve(&manifest, &named, Protocols::both()).ok())
}

fn line(forms: &[&str], action: &Action, settings: &Settings) -> Option<String> {
    plan(forms)
        .map(|plan| build(&plan, settings, stack_dir(), action, Environment::MacOs).join(" "))
}

/// A run aimed at another machine names it on the invocation, ahead of the
/// subcommand, where it is a global flag.
///
/// The endpoint beats whatever the shell exported, which is the whole reason it
/// is named here rather than left to the environment: Compose is a subprocess and
/// would inherit one, and an inherited endpoint is a second opinion about which
/// machine this run is about. Everything else is the invocation a local run
/// builds, unchanged.
#[test]
fn a_run_aimed_elsewhere_names_the_machine_ahead_of_the_subcommand() {
    let settings = Settings {
        docker: crate::ports::docker::Target::at(
            "ssh://media@nas.local",
            crate::ports::docker::Origin::Variable,
        ),
        ..Settings::default()
    };
    assert_eq!(
        line(&["library"], &Action::Up, &settings).as_deref(),
        Some(concat!(
            "docker --host ssh://media@nas.local compose ",
            "--project-name lemonfiber ",
            "--project-directory /opt/lemonfiber/stack ",
            "--file /opt/lemonfiber/stack/compose.yml ",
            "--profile media up --detach"
        ))
    );
}

#[test]
fn starts_a_form_detached_with_its_profiles() {
    assert_eq!(
        line(&["library"], &Action::Up, &Settings::default()).as_deref(),
        Some(concat!(
            "docker compose --project-name lemonfiber ",
            "--project-directory /opt/lemonfiber/stack ",
            "--file /opt/lemonfiber/stack/compose.yml ",
            "--profile media up --detach"
        ))
    );
}

/// The other half of switching image fetching off. A fetch asked for on its own
/// is refused before it reaches here; a start would fetch whatever is missing
/// without being told not to, and a setting that only stopped the first would be
/// a setting that means half of what it says.
#[test]
fn a_start_is_told_never_to_pull_where_fetching_is_switched_off() {
    let refusing = Settings {
        reaching: crate::config::Reaching::without(crate::config::REACH_REGISTRY_KEY),
        ..Settings::default()
    };
    let said = line(&["library"], &Action::Up, &refusing);
    assert_eq!(
        said.as_deref()
            .map(|command| command.ends_with("up --detach --pull never")),
        Some(true),
        "{said:?}"
    );
    let narrowed = line(
        &["library"],
        &Action::Start(vec!["jellyfin".to_owned()]),
        &refusing,
    );
    assert_eq!(
        narrowed
            .as_deref()
            .map(|command| command.ends_with("up --detach --pull never -- jellyfin")),
        Some(true),
        "{narrowed:?}"
    );
}

/// And a machine that allows fetching says nothing about pulling at all, so
/// Compose keeps its own default of fetching what is missing.
#[test]
fn a_start_that_may_fetch_is_told_nothing_about_pulling() {
    for action in [Action::Up, Action::Start(vec!["jellyfin".to_owned()])] {
        let said = line(&["library"], &action, &Settings::default());
        assert_eq!(
            said.as_deref().map(|command| command.contains("--pull")),
            Some(false),
            "{said:?}"
        );
    }
}

#[test]
fn the_same_request_always_produces_the_same_command() {
    let first = line(&["tv"], &Action::Up, &Settings::default());
    let second = line(&["tv"], &Action::Up, &Settings::default());
    assert_eq!(first, second);
    assert_eq!(
        first.as_deref().map(|command| command.contains(
            "--profile search --profile subs --profile torrent --profile tv --profile usenet"
        )),
        Some(true),
        "profiles are sorted: {first:?}"
    );
}

#[test]
fn naming_forms_in_a_different_order_produces_the_same_command() {
    assert_eq!(
        line(&["search", "library"], &Action::Up, &Settings::default()),
        line(&["library", "search"], &Action::Up, &Settings::default())
    );
}

#[test]
fn an_environment_file_is_passed_when_one_has_been_written() {
    let settings = Settings {
        env_file: Some(PathBuf::from("/home/op/.config/lemonfiber/.env")),
        ..Settings::default()
    };
    assert_eq!(
        line(&["library"], &Action::Up, &settings)
            .as_deref()
            .map(|command| command.contains("--env-file /home/op/.config/lemonfiber/.env --file")),
        Some(true)
    );
}

#[test]
fn overlays_are_layered_after_the_stack_s_own_file() {
    let settings = Settings {
        overlays: vec![PathBuf::from(
            "/opt/lemonfiber/stack/stacks/compose.storage.nas.yml",
        )],
        ..Settings::default()
    };
    assert_eq!(
        line(&["library"], &Action::Up, &settings)
            .as_deref()
            .map(|command| command.contains(concat!(
                "--file /opt/lemonfiber/stack/compose.yml ",
                "--file /opt/lemonfiber/stack/stacks/compose.storage.nas.yml"
            ))),
        Some(true)
    );
}

/// An installed plugin's document is layered, and after the operator's own
/// overlay: Compose takes the later file as the one that wins, and a stranger's
/// plugin is not entitled to override a choice the operator made.
#[test]
fn an_installed_plugin_s_document_is_layered_after_the_operator_s_own() {
    let settings = Settings {
        overlays: vec![PathBuf::from(
            "/opt/lemonfiber/stack/stacks/compose.storage.nas.yml",
        )],
        plugins: vec!["komga".to_owned()],
        ..Settings::default()
    };
    assert_eq!(
        line(&["library"], &Action::Up, &settings)
            .as_deref()
            .map(|command| command.contains(concat!(
                "--file /opt/lemonfiber/stack/stacks/compose.storage.nas.yml ",
                "--file /opt/lemonfiber/stack/compose/plugins/komga.yml"
            ))),
        Some(true)
    );
}

/// The document is joined against the directory this invocation calls the project
/// root, so an operator's own stack gets the document inside it rather than inside
/// the one lemonfiber would have materialised.
#[test]
fn a_plugin_s_document_is_joined_against_the_root_the_invocation_names() {
    let settings = Settings {
        plugins: vec!["komga".to_owned()],
        ..Settings::default()
    };
    let theirs = Path::new("/srv/their-own-stack");
    let command = plan(&["library"])
        .map(|plan| build(&plan, &settings, theirs, &Action::Up, Environment::MacOs).join(" "));
    assert_eq!(
        command
            .as_deref()
            .map(|line| line.contains("--file /srv/their-own-stack/compose/plugins/komga.yml")),
        Some(true)
    );
}

/// A machine with nothing installed layers nothing, so the invocation a stack
/// without plugins produces is the one it always produced.
#[test]
fn a_machine_with_no_plugins_layers_no_extra_documents() {
    let settings = Settings::default();
    assert_eq!(
        line(&["library"], &Action::Up, &settings)
            .as_deref()
            .map(|command| command.contains("compose/plugins/")),
        Some(false)
    );
}

/// What an action is called is what a report says it did, and every one of them
/// has to have an answer — a word missing here is a run an operator cannot name.
#[test]
fn every_action_says_what_it_is_called() {
    assert_eq!(Action::Up.name(), "up");
    assert_eq!(Action::Start(Vec::new()).name(), "up");
    assert_eq!(Action::Down.name(), "down");
    assert_eq!(Action::Stop(Vec::new()).name(), "stop");
    assert_eq!(Action::Remove(Vec::new()).name(), "rm");
    assert_eq!(Action::Restart(Vec::new()).name(), "restart");
    assert_eq!(Action::Pull.name(), "pull");
    assert_eq!(Action::Config.name(), "config");
}

#[test]
fn each_action_becomes_its_own_subcommand() {
    let settings = Settings::default();
    let ending = |action: &Action| {
        line(&["library"], action, &settings).and_then(|command| {
            command
                .split("--profile media ")
                .nth(1)
                .map(ToOwned::to_owned)
        })
    };

    assert_eq!(ending(&Action::Up).as_deref(), Some("up --detach"));
    assert_eq!(ending(&Action::Down).as_deref(), Some("down"));
    assert_eq!(ending(&Action::Stop(Vec::new())).as_deref(), Some("stop"));
    assert_eq!(
        ending(&Action::Remove(vec!["komga".to_owned()])).as_deref(),
        Some("rm --force --stop -- komga"),
        "a container has to be stopped before it can be removed, and the question \
         it would otherwise ask is put to a terminal nobody is watching"
    );
    assert_eq!(ending(&Action::Pull).as_deref(), Some("pull"));
    assert_eq!(ending(&Action::Config).as_deref(), Some("config"));
    assert_eq!(
        ending(&Action::Restart(vec!["sonarr".to_owned()])).as_deref(),
        Some("restart -- sonarr"),
        "named services are fenced off from option parsing"
    );
    assert_eq!(
        ending(&Action::Restart(Vec::new())).as_deref(),
        Some("restart"),
        "restarting nothing in particular restarts the form"
    );
    assert_eq!(
        ending(&Action::Stop(vec!["qbittorrent".to_owned()])).as_deref(),
        Some("stop -- qbittorrent"),
        "stopping part of what is up names only that part, fenced the same way"
    );
    assert_eq!(
        ending(&Action::Start(vec!["sonarr".to_owned()])).as_deref(),
        Some("up --detach -- sonarr"),
        "a start aimed at services keeps the detach and fences the names after it"
    );
}

#[test]
fn every_action_reports_the_name_it_runs_under() {
    for (action, name) in [
        (Action::Up, "up"),
        (Action::Start(Vec::new()), "up"),
        (Action::Down, "down"),
        (Action::Stop(Vec::new()), "stop"),
        (Action::Restart(Vec::new()), "restart"),
        (Action::Pull, "pull"),
        (Action::Config, "config"),
    ] {
        assert_eq!(action.name(), name);
        assert_eq!(
            action.argv(true).first().map(String::as_str),
            Some(name),
            "the name and the subcommand must not drift apart"
        );
    }
}

#[test]
fn the_project_name_is_what_correlates_containers_back_to_services() {
    let settings = Settings {
        project: "housemedia".to_owned(),
        ..Settings::default()
    };
    assert_eq!(
        line(&["library"], &Action::Up, &settings)
            .as_deref()
            .map(|command| command.contains("--project-name housemedia")),
        Some(true)
    );
}
