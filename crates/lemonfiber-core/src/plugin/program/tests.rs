use std::time::Duration;

use lemonfiber_fixtures::proving::Proving;
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_plugin::{Expect, Manifest};

use super::super::installed::{Installed, Placed};
use super::super::recorded::RunResult;
use super::super::Verdict;
use super::{contained, limit, performed, recorded, settled, verdict, Came, Operator, Starting};
use crate::ports::program::{Configuration, Unran};

/// A plugin whose one service has no port, proved by running its own program.
pub(crate) const RUNNING: &str = r#"
schema_version = 1

[plugin]
id          = "quality"
name        = "Quality sync"
version     = "1.0.0"
description = "Keeps quality profiles in step with the guides"
without_it  = "Profiles drift from the guides"
upstream    = "https://github.com/recyclarr/recyclarr"
license     = "MIT"
forms       = ["library"]

[[service]]
id            = "qualitysync"
name          = "Recyclarr"
image         = "ghcr.io/recyclarr/recyclarr"
digest        = "sha256:9c6c74d52cfc6f740a5a3624aa83867e171bb70b90453ddb7a4048b8c80445f9"
tag           = "8.7.3"
criticality   = "optional"
config_path   = "/config"
user_mode     = "uid"
posture       = "confined"
mem_limit_mib = 256
env           = { CRON_SCHEDULE = "@daily", LOUD = true, TRIES = 3, API = { setting = "quality:api", default = "x" }, KEY = { secret = "key" } }

[[secret]]
id  = "key"
of  = "qualitysync"
why = "It is asked with it."

[[proof]]
id      = "recyclarr.runs"
title   = "The pinned quality sync starts as the service runs it"
run     = { args = ["--version"], timeout_s = 45 }
expect  = { exit = 0, stdout_starts_with = "v8.7.3 (" }
fixture = "recordings/recyclarr/runs-version.json"
why     = "It has no port."
"#;

fn placed(text: &str) -> Option<Placed> {
    let manifest = Manifest::from_toml(text);
    assert!(manifest.is_ok(), "the fixture reads: {manifest:?}");
    manifest
        .ok()
        .map(|manifest| Installed::of(&manifest))
        .and_then(|installed| installed.services.into_iter().next())
}

fn run_of(args: &[&str], timeout_s: Option<u32>) -> lemonfiber_plugin::Run {
    lemonfiber_plugin::Run {
        args: args.iter().map(|one| (*one).to_owned()).collect(),
        timeout_s,
    }
}

fn operator() -> Operator {
    Operator {
        zone: "Europe/Amsterdam".to_owned(),
        ids: ("501".to_owned(), "20".to_owned()),
    }
}

#[test]
fn a_run_is_given_what_it_declares_within_the_bounds() {
    assert_eq!(limit(&run_of(&["-v"], None)), 30);
    assert_eq!(limit(&run_of(&["-v"], Some(45))), 45);
    assert_eq!(limit(&run_of(&["-v"], Some(0))), 1);
    assert_eq!(limit(&run_of(&["-v"], Some(121))), 120);
}

#[test]
fn a_confined_run_has_no_network_capability_or_writable_root_and_only_the_constants() {
    let copy = std::path::Path::new("/tmp/copy");
    let asked = placed(RUNNING).map(|placed| {
        settled(
            &placed,
            &run_of(&["--version"], Some(45)),
            &operator(),
            Some(copy),
        )
    });
    let Some(asked) = asked else {
        return;
    };
    assert_eq!(
        asked.image,
        "ghcr.io/recyclarr/recyclarr@sha256:9c6c74d52cfc6f740a5a3624aa83867e171bb70b90453ddb7a4048b8c80445f9"
    );
    assert_eq!(asked.args, vec!["--version".to_owned()]);
    assert_eq!(
        asked.env,
        vec![
            "CRON_SCHEDULE=@daily".to_owned(),
            "LOUD=true".to_owned(),
            "TRIES=3".to_owned(),
            "TZ=Europe/Amsterdam".to_owned(),
        ]
    );
    assert_eq!(asked.user.as_deref(), Some("501:20"));
    assert_eq!(asked.cap_drop, vec!["ALL".to_owned()]);
    assert_eq!(asked.cap_add, Vec::<String>::new());
    assert_eq!(
        asked.security_opt,
        vec!["no-new-privileges:true".to_owned()]
    );
    assert_eq!(asked.pids_limit, 1024);
    assert!(asked.read_only);
    assert_eq!(asked.memory, Some(256 * 1024 * 1024));
    assert_eq!(
        asked.configuration,
        Some(Configuration {
            source: copy.to_path_buf(),
            target: "/config".to_owned(),
        })
    );
    assert_eq!(asked.within, Duration::from_secs(45));
    assert_eq!(asked.read_at_most, 64 * 1024);
}

#[test]
fn a_standard_run_is_given_back_six_capabilities_and_a_writable_root() {
    let text = RUNNING.replace("posture       = \"confined\"\n", "");
    let Some(asked) =
        placed(&text).map(|placed| settled(&placed, &run_of(&["-v"], None), &operator(), None))
    else {
        return;
    };
    assert_eq!(
        asked.cap_add,
        [
            "CHOWN",
            "DAC_OVERRIDE",
            "FOWNER",
            "SETGID",
            "SETUID",
            "KILL"
        ]
        .map(str::to_owned)
    );
    assert!(!asked.read_only);
    assert_eq!(asked.configuration, None);
}

/// The service of [`RUNNING`], standing rather than confined, written with `shape`.
fn shaped(shape: &str) -> Option<Placed> {
    placed(&RUNNING.replace(
        "posture       = \"confined\"\n",
        &format!("shape         = \"{shape}\"\n"),
    ))
}

#[test]
fn an_egress_guard_is_given_the_network_capability_and_the_tunnel_device_it_is_written_with() {
    let Some(asked) = shaped("egress-guard").map(|placed| contained(&placed, &operator())) else {
        unreachable!("the fixture reads");
    };
    assert!(
        asked.cap_add.iter().any(|one| one == "NET_ADMIN"),
        "{:?}",
        asked.cap_add
    );
    assert_eq!(asked.devices, vec!["/dev/net/tun:/dev/net/tun".to_owned()]);
}

#[test]
fn a_render_shape_passes_no_device_the_host_is_not_asked_about() {
    let Some(asked) = shaped("render").map(|placed| contained(&placed, &operator())) else {
        unreachable!("the fixture reads");
    };
    assert!(asked.devices.is_empty(), "{:?}", asked.devices);
    assert!(!asked.cap_add.iter().any(|one| one == "NET_ADMIN"));
}

#[test]
fn who_a_run_is_follows_the_services_user_mode() {
    let as_env = RUNNING.replace("user_mode     = \"uid\"", "user_mode     = \"puid-env\"");
    let env =
        placed(&as_env).map(|placed| settled(&placed, &run_of(&["-v"], None), &operator(), None));
    assert_eq!(env.as_ref().and_then(|asked| asked.user.clone()), None);
    assert!(env.is_some_and(|asked| {
        asked
            .env
            .ends_with(&["PUID=501".to_owned(), "PGID=20".to_owned()])
    }));
    let as_image = RUNNING.replace("user_mode     = \"uid\"\n", "user_mode     = \"image\"\n");
    let image =
        placed(&as_image).map(|placed| settled(&placed, &run_of(&["-v"], None), &operator(), None));
    assert!(image.is_some_and(
        |asked| asked.user.is_none() && !asked.env.iter().any(|one| one.starts_with("PUID"))
    ));
}

fn expecting() -> Expect {
    Expect {
        exit: Some(0),
        stdout_starts_with: Some("v8.7.3 (".to_owned()),
        ..Expect::default()
    }
}

#[tokio::test]
async fn a_run_that_said_what_it_declares_passes_and_its_copy_is_gone() {
    let source = Scratch::new("program-passes");
    let _ = std::fs::create_dir_all(source.join("config/qualitysync"));
    let _ = std::fs::write(source.join("config/qualitysync/recyclarr.yml"), "x");
    let program = Proving::exiting(0, "v8.7.3 (Branch.master)\n");
    let Some(placed) = placed(RUNNING) else {
        return;
    };
    let came = performed(
        program.as_ref(),
        &placed,
        &run_of(&["--version"], None),
        &operator(),
        Some(Starting {
            root: &source,
            service: "qualitysync",
        }),
        &lemonfiber_fixtures::ports::Chance::cycling(),
    )
    .await;
    assert_eq!(verdict(&came, &expecting()), Verdict::Passed);
    let mounted = program
        .asked()
        .first()
        .and_then(|one| one.configuration.clone());
    assert!(mounted.is_some_and(|mounted| !mounted.source.exists()));
}

#[tokio::test]
async fn a_run_that_overran_fails_naming_its_limit() {
    let program = Proving::overrunning("v8.7.3 (");
    let Some(placed) = placed(RUNNING) else {
        return;
    };
    let came = performed(
        program.as_ref(),
        &placed,
        &run_of(&["sync"], Some(5)),
        &operator(),
        None,
        &lemonfiber_fixtures::ports::Chance::cycling(),
    )
    .await;
    assert_eq!(came, Came::Overran(5));
    assert_eq!(
        verdict(&came, &expecting()),
        Verdict::Failed {
            faults: vec![
                "it was still running at its limit of 5 seconds, and was stopped".to_owned()
            ]
        }
    );
}

#[tokio::test]
async fn a_run_the_engine_could_not_make_is_unproven_naming_why() {
    let Some(placed) = placed(RUNNING) else {
        return;
    };
    for (unran, words) in [
        (
            Unran::Unreachable {
                reason: "\u{1b}[2Jgone".to_owned(),
            },
            "could not be reached: [2Jgone",
        ),
        (
            Unran::NoImage {
                image: "a@sha256:b".to_owned(),
            },
            "pulls no image",
        ),
        (
            Unran::Refused {
                reason: "no".to_owned(),
            },
            "refused the run",
        ),
    ] {
        let came = performed(
            Proving::answering(Err(unran)).as_ref(),
            &placed,
            &run_of(&["-v"], None),
            &operator(),
            None,
            &lemonfiber_fixtures::ports::Chance::cycling(),
        )
        .await;
        let said = verdict(&came, &expecting());
        assert!(
            matches!(&said, Verdict::Unproven { why } if why.contains(words)),
            "{words}: {said:?}"
        );
    }
}

#[tokio::test]
async fn a_run_whose_starting_files_are_refused_is_unproven_and_never_made() {
    let source = Scratch::new("program-linked");
    let _ = std::fs::create_dir_all(source.join("config"));
    #[cfg(unix)]
    let _ = std::os::unix::fs::symlink("/etc", source.join("config/qualitysync"));
    let program = Proving::exiting(0, "v8.7.3 (");
    let Some(placed) = placed(RUNNING) else {
        return;
    };
    let came = performed(
        program.as_ref(),
        &placed,
        &run_of(&["-v"], None),
        &operator(),
        Some(Starting {
            root: &source,
            service: "qualitysync",
        }),
        &lemonfiber_fixtures::ports::Chance::cycling(),
    )
    .await;
    #[cfg(unix)]
    {
        assert!(matches!(came, Came::Unproven(why) if why.contains("is a link")));
        assert!(program.asked().is_empty());
    }
}

#[test]
fn what_a_run_wrote_is_read_as_text_and_failed_where_it_says_otherwise() {
    let came = Came::Ended(RunResult {
        exit: 1,
        stdout_starts_with: "error".to_owned(),
    });
    let said = verdict(&came, &expecting());
    assert_eq!(
        said,
        Verdict::Failed {
            faults: vec![
                "it exited 1 where it declares 0".to_owned(),
                "its output begins \"error\", and it declares \"v8.7.3 (\"".to_owned(),
            ]
        }
    );
}

#[test]
fn a_recording_keeps_the_first_kibibyte_cut_where_a_character_ends() {
    let long = RunResult {
        exit: 0,
        stdout_starts_with: format!("{}é{}", "a".repeat(1023), "b".repeat(10)),
    };
    let kept = recorded(&long);
    assert_eq!(kept.stdout_starts_with, "a".repeat(1023));
    let exact = RunResult {
        exit: 3,
        stdout_starts_with: "a".repeat(1024),
    };
    assert_eq!(recorded(&exact), exact);
    let short = RunResult {
        exit: 0,
        stdout_starts_with: "v8\n".to_owned(),
    };
    assert_eq!(recorded(&short), short);
}

#[test]
fn the_operator_is_what_this_machines_settings_write_each_falling_back_as_the_entry_does() {
    let machine = Scratch::new("program-operator");
    let env = machine.join(".env");
    let _ = std::fs::write(&env, "TZ=America/New_York\n");
    let set = crate::test_support::a_context()
        .settings(crate::config::Settings {
            env_file: Some(env.clone()),
            service_user: Some((501, 20)),
            ..crate::config::Settings::default()
        })
        .build();
    assert_eq!(
        Operator::of(&set),
        Operator {
            zone: "America/New_York".to_owned(),
            ids: ("501".to_owned(), "20".to_owned()),
        }
    );
    let _ = std::fs::write(&env, "TZ=  \n");
    let blank = crate::test_support::a_context()
        .settings(crate::config::Settings {
            env_file: Some(env),
            ..crate::config::Settings::default()
        })
        .build();
    assert_eq!(Operator::of(&blank), Operator::recording());
}
