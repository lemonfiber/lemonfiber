use std::path::PathBuf;
use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};

use crate::app::{answered_under, dispatch, rehearsal, Command, Ctx, Outcome, Rehearsal};
use crate::error::codes::plugin::{KEY_UNREADABLE, UNREADABLE};
use crate::error::Problem;
use crate::model::kind;
use crate::plugin::{Provenance, Retaking, Took};
use crate::ports::program::Program;
use crate::ports::registry::Registry;
use crate::test_support::a_context;

use super::Asked;

const WHOLE: &str = r#"
schema_version = 1

[plugin]
id          = "kavita"
name        = "Kavita"
version     = "1.0.0"
description = "Reads comics in a browser"
without_it  = "Comics stay folders of images"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "kavita"
name        = "Kavita"
image       = "example.invalid/kavita"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.0.0"
criticality = "enhancing"
"#;

const PUBLIC_KEY: &str = "-----BEGIN PUBLIC KEY-----\n\
        MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEXO0Zk0m0jt9dqnbvJx6tQYFThLEC\n\
        E9HmhZsMHLTKQ2u/0O4bnfsX+zEr1Nru7O6Yf+MbtXeJ+rNJ4BBBzbf9bA==\n\
        -----END PUBLIC KEY-----\n";

fn unpinned() -> String {
    WHOLE.replace(
        "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
        "sha256:not-a-digest",
    )
}

fn running() -> String {
    format!(
        "{WHOLE}\n[[proof]]\nid      = \"kavita.runs\"\ntitle   = \"It starts\"\n\
         run     = {{ args = [\"--version\"] }}\n\
         expect  = {{ exit = 0, stdout_starts_with = \"v1\" }}\n\
         fixture = \"recordings/kavita/runs.json\"\nwhy     = \"It starts.\"\n"
    )
}

fn source(named: &str, manifest: &str) -> PathBuf {
    let at = lemonfiber_fixtures::scratch::Scratch::unmade(&format!("authoring-{named}")).kept();
    let _ = std::fs::create_dir_all(&at);
    let _ = std::fs::write(at.join("plugin.toml"), manifest);
    at
}

fn written(named: &str, text: &str) -> PathBuf {
    let at = lemonfiber_fixtures::scratch::Scratch::new(&format!("authoring-{named}")).kept();
    let file = at.join("key.pem");
    let _ = std::fs::write(&file, text);
    file
}

fn nowhere(named: &str) -> PathBuf {
    lemonfiber_fixtures::scratch::Scratch::unmade(&format!("authoring-{named}")).kept()
}

fn registry_answering(status: u16, body: &'static str) -> Arc<dyn Registry> {
    Arc::new(lemonfiber_adapters::registry::Oci::new(Fake::always(
        Answer::reply(status, body),
    )))
}

fn registry_offering(payload: &'static str) -> Arc<dyn Registry> {
    const MANIFEST: &str = r#"{"schemaVersion":2,"layers":[
            {"digest":"sha256:aaa","annotations":{"dev.cosignproject.cosign/signature":"AQID"}}
        ]}"#;
    Arc::new(lemonfiber_adapters::registry::Oci::new(Fake::by_path(
        vec![
            ("/manifests/", Answer::reply(200, MANIFEST)),
            ("/blobs/", Answer::reply(200, payload)),
        ],
    )))
}

fn authoring(program: Arc<dyn Program>, registry: Arc<dyn Registry>) -> Ctx {
    a_context()
        .filesystem(Arc::new(lemonfiber_adapters::Disk))
        .build()
        .with_program(program)
        .with_registry(registry)
}

fn plain() -> Ctx {
    authoring(
        lemonfiber_fixtures::proving::Proving::unreachable(),
        registry_answering(404, "{}"),
    )
}

async fn answered(asked: Asked, ctx: &Ctx) -> Result<Outcome, Box<Problem>> {
    dispatch(Command::Authoring(asked), ctx).await
}

#[tokio::test]
async fn a_plugin_nothing_refuses_answers_under_its_claims_kind_and_is_installable() {
    let outcome = answered(
        Asked::Claims {
            path: source("whole", WHOLE),
        },
        &plain(),
    )
    .await;

    let Ok(Outcome::PluginClaims(claimed)) = outcome else {
        unreachable!("claims answer as plugin-claims, got {outcome:?}");
    };
    assert_eq!(claimed.id, "kavita");
    assert!(claimed.installable, "{claimed:?}");
    assert_eq!(Outcome::PluginClaims(claimed).kind(), kind::PLUGIN_CLAIMS);
}

#[tokio::test]
async fn a_refused_manifest_is_an_answer_that_says_it_would_not_install() {
    let outcome = answered(
        Asked::Claims {
            path: source("unpinned", &unpinned()),
        },
        &plain(),
    )
    .await;

    let Ok(Outcome::PluginClaims(claimed)) = outcome else {
        unreachable!("a refused manifest is still a verdict, got {outcome:?}");
    };
    assert!(!claimed.installable);
    assert!(!claimed.refusals.is_empty());
}

#[tokio::test]
async fn a_directory_and_the_manifest_inside_it_are_the_same_plugin() {
    let at = source("either-way", WHOLE);

    let directory = answered(Asked::Claims { path: at.clone() }, &plain()).await;
    let file = answered(
        Asked::Claims {
            path: at.join("plugin.toml"),
        },
        &plain(),
    )
    .await;

    assert_eq!(directory, file);
}

#[tokio::test]
async fn conforming_answers_whether_the_plugin_would_fill_what_it_speaks() {
    let whole = answered(
        Asked::Conform {
            path: source("conform-whole", WHOLE),
        },
        &plain(),
    )
    .await;
    let Ok(Outcome::PluginConformance(whole)) = whole else {
        unreachable!("conformance answers as plugin-conformance, got {whole:?}");
    };
    assert!(whole.conforms && whole.cases.is_empty(), "{whole:?}");

    let refused = answered(
        Asked::Conform {
            path: source("conform-unpinned", &unpinned()),
        },
        &plain(),
    )
    .await;
    let Ok(Outcome::PluginConformance(refused)) = refused else {
        unreachable!("a refused manifest is still a verdict, got {refused:?}");
    };
    assert!(!refused.conforms && !refused.refusals.is_empty());
}

#[tokio::test]
async fn a_path_holding_no_plugin_is_refused_with_the_unreadable_code() {
    for asked in [
        Asked::Claims {
            path: nowhere("none-claims"),
        },
        Asked::Conform {
            path: nowhere("none-conform"),
        },
        Asked::Provenance {
            path: nowhere("none-provenance"),
            keys: Vec::new(),
        },
        Asked::Record {
            path: nowhere("none-record"),
            taking: Retaking::default(),
        },
    ] {
        let refused = answered(asked.clone(), &plain()).await;

        let Err(problem) = refused else {
            unreachable!("{asked:?} holds no plugin and was answered: {refused:?}");
        };
        assert_eq!(problem.code, UNREADABLE, "{asked:?}");
        assert!(
            problem
                .detail
                .as_deref()
                .is_some_and(|why| why.contains("plugin.toml")),
            "{problem:?}"
        );
    }
}

#[tokio::test]
async fn an_image_no_registry_offers_a_signature_for_is_unproven_and_allowed() {
    let outcome = answered(
        Asked::Provenance {
            path: source("unproven", WHOLE),
            keys: Vec::new(),
        },
        &plain(),
    )
    .await;

    let Ok(Outcome::PluginProvenance(vouched)) = outcome else {
        unreachable!("provenance answers as plugin-provenance, got {outcome:?}");
    };
    assert!(vouched.installable);
    assert!(vouched
        .images
        .iter()
        .all(|one| matches!(one.held, Provenance::Unproven { .. })));
}

#[tokio::test]
async fn an_image_whose_signature_does_not_hold_would_not_be_installed() {
    let elsewhere = r#"{"critical":{"image":{"docker-manifest-digest":"sha256:0000000000000000000000000000000000000000000000000000000000000000"}}}"#;
    let ctx = authoring(
        lemonfiber_fixtures::proving::Proving::unreachable(),
        registry_offering(elsewhere),
    );

    let outcome = answered(
        Asked::Provenance {
            path: source("refused", WHOLE),
            keys: vec![written("refusing-key", PUBLIC_KEY)],
        },
        &ctx,
    )
    .await;

    let Ok(Outcome::PluginProvenance(vouched)) = outcome else {
        unreachable!("provenance answers as plugin-provenance, got {outcome:?}");
    };
    assert!(!vouched.installable, "{vouched:?}");
}

#[tokio::test]
async fn a_key_that_cannot_be_used_stops_the_verification_naming_it() {
    for (key, why) in [
        (PathBuf::from("/nowhere/signing.pem"), "could not be read"),
        (written("gibberish", "hello"), "PEM"),
    ] {
        let refused = answered(
            Asked::Provenance {
                path: source("bad-key", WHOLE),
                keys: vec![key.clone()],
            },
            &plain(),
        )
        .await;

        let Err(problem) = refused else {
            unreachable!("an unusable key was dropped rather than refused: {refused:?}");
        };
        assert_eq!(problem.code, KEY_UNREADABLE);
        assert!(
            problem.summary.contains(&key.display().to_string()),
            "{problem:?}"
        );
        assert!(
            problem
                .detail
                .as_deref()
                .is_some_and(|said| said.contains(why)),
            "{problem:?}"
        );
    }
}

#[tokio::test]
async fn recording_writes_what_the_run_said_and_reproduces_it_once_noted() {
    let at = source("record", &running());
    let ctx = authoring(
        lemonfiber_fixtures::proving::Proving::exiting(0, "v1.2\n"),
        registry_answering(404, "{}"),
    );
    let asked = Asked::Record {
        path: at.clone(),
        taking: Retaking::default(),
    };

    let wrote = answered(asked.clone(), &ctx).await;
    let Ok(Outcome::PluginRecording(wrote)) = wrote else {
        unreachable!("recording answers as plugin-recording, got {wrote:?}");
    };
    assert!(!wrote.holds, "a recording with no note fails the run");
    let kept = at.join("recordings/kavita/runs.json");
    let text = std::fs::read_to_string(&kept).unwrap_or_default();
    assert!(
        text.contains("\"stdout_starts_with\": \"v1.2\\n\""),
        "{text}"
    );

    let _ = std::fs::write(
        &kept,
        text.replace("\"note\": \"\"", "\"note\": \"It starts.\""),
    );
    let noted = answered(asked, &ctx).await;
    let Ok(Outcome::PluginRecording(noted)) = noted else {
        unreachable!("recording answers as plugin-recording, got {noted:?}");
    };
    assert!(noted.holds, "{noted:?}");
    assert!(noted
        .recordings
        .iter()
        .all(|one| one.took == Took::Reproduced));
}

#[tokio::test]
async fn a_run_proof_that_cannot_be_made_is_not_recorded() {
    let outcome = answered(
        Asked::Record {
            path: source("record-unmade", &running()),
            taking: Retaking::default(),
        },
        &plain(),
    )
    .await;

    let Ok(Outcome::PluginRecording(unmade)) = outcome else {
        unreachable!("recording answers as plugin-recording, got {outcome:?}");
    };
    assert!(!unmade.holds);
    assert!(unmade
        .recordings
        .iter()
        .any(|one| matches!(one.took, Took::Unrecorded { .. })));
}

#[tokio::test]
async fn a_rehearsed_recording_that_would_write_is_refused_before_it_runs() {
    let mut ctx = plain();
    ctx.dry_run = true;

    let refused = answered(
        Asked::Record {
            path: source("record-rehearsed", &running()),
            taking: Retaking::default(),
        },
        &ctx,
    )
    .await;

    assert!(refused.is_err(), "{refused:?}");
}

#[test]
fn only_a_recording_that_writes_is_refused_a_rehearsal() {
    let path = PathBuf::from("plugin");
    let check = Retaking {
        check: true,
        ..Retaking::default()
    };
    let reads = [
        Asked::Claims { path: path.clone() },
        Asked::Conform { path: path.clone() },
        Asked::Provenance {
            path: path.clone(),
            keys: Vec::new(),
        },
        Asked::Record {
            path: path.clone(),
            taking: check,
        },
    ];
    for asked in reads {
        let rehearsal = rehearsal::asked(&Command::Authoring(asked.clone())).rehearsal;
        assert!(matches!(rehearsal, Rehearsal::Reads), "{asked:?}");
    }

    let writing = Command::Authoring(Asked::Record {
        path,
        taking: Retaking::default(),
    });
    assert!(matches!(
        rehearsal::asked(&writing).rehearsal,
        Rehearsal::Cannot(_)
    ));
}

#[test]
fn each_question_names_the_kind_it_is_answered_under() {
    let path = PathBuf::from("plugin");
    let asked = [
        (Asked::Claims { path: path.clone() }, kind::PLUGIN_CLAIMS),
        (
            Asked::Conform { path: path.clone() },
            kind::PLUGIN_CONFORMANCE,
        ),
        (
            Asked::Provenance {
                path: path.clone(),
                keys: Vec::new(),
            },
            kind::PLUGIN_PROVENANCE,
        ),
        (
            Asked::Record {
                path,
                taking: Retaking::default(),
            },
            kind::PLUGIN_RECORDING,
        ),
    ];
    for (asked, kind) in asked {
        assert_eq!(answered_under(&Command::Authoring(asked)), &[kind]);
    }
}
