use std::sync::Arc;

use lemonfiber_fixtures::proving::Proving;
use lemonfiber_fixtures::support::Reporting;
use lemonfiber_manifest::container::Network;

use super::{answering, read, Asked};
use crate::app::Ctx;
use crate::plugin::{Placed, Verdict};
use crate::ports::docker::{Health, Lifecycle};
use crate::ports::program::{Joined, Program};

/// The service asked, which publishes no port.
const SOLVER: &str = "solver";

/// The port it listens on inside its own network.
const LISTENS: u16 = 8191;

/// What the service answers, as `fence ask` prints it.
const ANSWERED: &str = "200\napplication/json\n{\"paths\":{\"/v1\":{}}}";

/// The request the proof asks.
fn asking(method: &str, path: &str) -> lemonfiber_plugin::Request {
    lemonfiber_plugin::Request {
        method: method.to_owned(),
        path: path.to_owned(),
        accept: None,
        varies: std::collections::BTreeMap::new(),
    }
}

/// What the proof expects of the answer.
fn expecting(status: u16) -> lemonfiber_plugin::Expect {
    lemonfiber_plugin::Expect {
        status: Some(status),
        json_has_keys: Some(vec!["/paths".to_owned()]),
        ..lemonfiber_plugin::Expect::default()
    }
}

/// The service, listening on [`LISTENS`], fenced where `fenced`.
fn solver(fenced: bool) -> Placed {
    let mut placed = crate::test_support::a_placed(SOLVER, &[], None);
    placed.listens = Some(LISTENS);
    placed.fields.network = fenced.then_some(Network {
        via: None,
        fenced: true,
        internal: false,
        egress_for: None,
    });
    placed
}

/// A machine whose engine runs `running` and runs the fence as `program` answers.
fn machine(running: &[&str], program: &Arc<Proving>) -> Ctx {
    crate::test_support::a_context()
        .engine(Arc::new(Reporting::holding(
            running,
            Lifecycle::Running,
            Health::Healthy,
        )))
        .build()
        .with_program(Arc::clone(program) as Arc<dyn Program>)
}

async fn asked(ctx: &Ctx, placed: &Placed, request: &lemonfiber_plugin::Request) -> Verdict {
    let expect = expecting(200);
    let asked = Asked {
        request,
        expect: &expect,
        image: lemonfiber_bundled::FENCE,
    };
    answering(ctx, placed, &asked, ctx.seams.clock.now()).await
}

#[tokio::test]
async fn a_fenced_service_is_asked_inside_its_fences_network_by_a_probe_holding_nothing() {
    let program = Proving::exiting(0, ANSWERED);
    let fence = crate::fence::of(SOLVER);
    let ctx = machine(&[SOLVER, &fence], &program);

    let verdict = asked(&ctx, &solver(true), &asking("get", "/openapi.json")).await;

    assert_eq!(verdict, Verdict::Passed);
    let runs = program.asked();
    let [run] = runs.as_slice() else {
        unreachable!("one probe was run: {runs:?}");
    };
    assert_eq!(Some(run.image.as_str()), lemonfiber_bundled::FENCE);
    assert_eq!(run.args, ["ask", "GET", "8191", "/openapi.json"]);
    assert_eq!(run.network, Joined::Namespace(format!("id-{fence}")));
    assert!(run.read_only && run.cap_add.is_empty() && run.bound.is_none());
    assert_eq!(run.cap_drop, ["ALL"]);
    assert_eq!(
        run.memory,
        Some(i64::from(crate::fence::MEMORY_MIB) * crate::plugin::program::MEBIBYTE)
    );
}

#[tokio::test]
async fn a_service_with_no_fence_is_asked_inside_its_own_network() {
    let program = Proving::exiting(0, ANSWERED);
    let ctx = machine(&[SOLVER], &program);

    let verdict = asked(&ctx, &solver(false), &asking("GET", "/")).await;

    assert_eq!(verdict, Verdict::Passed);
    let runs = program.asked();
    assert_eq!(
        runs.first().map(|run| run.network.clone()),
        Some(Joined::Namespace(format!("id-{SOLVER}")))
    );
}

#[tokio::test]
async fn an_answer_that_does_not_hold_fails_naming_what_it_said() {
    let program = Proving::exiting(0, "401\napplication/json\n{\"detail\":\"no\"}");
    let ctx = machine(&[SOLVER], &program);

    let verdict = asked(&ctx, &solver(false), &asking("GET", "/")).await;

    assert!(matches!(verdict, Verdict::Failed { .. }), "{verdict:?}");
}

#[tokio::test]
async fn a_service_not_running_or_a_probe_that_fails_is_unproven_saying_why() {
    for (running, program, said) in [
        (
            Vec::new(),
            Proving::exiting(0, ANSWERED),
            "solver is not running",
        ),
        (
            vec![SOLVER],
            Proving::exiting(1, ""),
            "exited 1 rather than answering",
        ),
        (
            vec![SOLVER],
            Proving::exiting(0, "no status\n"),
            "opens with no status",
        ),
        (vec![SOLVER], Proving::unreachable(), "was not asked"),
    ] {
        let ctx = machine(&running, &program);
        let verdict = asked(&ctx, &solver(false), &asking("GET", "/")).await;
        assert!(
            matches!(&verdict, Verdict::Unproven { why } if why.contains(said)),
            "{said}: {verdict:?}"
        );
    }
}

#[tokio::test]
async fn a_request_that_cannot_be_put_or_a_service_listening_nowhere_runs_no_probe() {
    let program = Proving::exiting(0, ANSWERED);
    let ctx = machine(&[SOLVER], &program);
    let mut nowhere = solver(false);
    nowhere.listens = None;

    for (placed, request, said) in [
        (solver(false), asking("BREW", "/"), "is not a method"),
        (
            solver(false),
            asking("GET", "@elsewhere/"),
            "is not a route",
        ),
        (nowhere, asking("GET", "/"), "declares none it listens on"),
    ] {
        let verdict = asked(&ctx, &placed, &request).await;
        assert!(
            matches!(&verdict, Verdict::Unproven { why } if why.contains(said)),
            "{said}: {verdict:?}"
        );
    }
    assert!(program.asked().is_empty());
}

#[test]
fn what_the_fence_prints_is_read_as_status_content_type_and_body() {
    let read = read(b"302\n\n");
    assert_eq!(
        read.map(|response| (response.status, response.headers, response.body)),
        Ok((302, Vec::new(), String::new()))
    );
    let read = super::read(ANSWERED.as_bytes());
    assert_eq!(
        read.map(|response| (response.status, response.headers, response.body)),
        Ok((
            200,
            vec![("content-type".to_owned(), "application/json".to_owned())],
            "{\"paths\":{\"/v1\":{}}}".to_owned()
        ))
    );
    assert!(super::read(b"200").is_err());
    assert!(super::read(b"").is_err());
}

/// Every request proof the bundle pins of a service that publishes no port names a service
/// that listens on one inside its network, so each is asked there rather than left with
/// nowhere to ask it: the egress guard's, the archive extractor's and each challenge
/// solver's among them.
#[test]
fn every_bundled_proof_of_a_service_publishing_nothing_names_one_it_can_be_asked_at() {
    let mut asked_inside = Vec::new();
    for pinned in lemonfiber_bundled::PINNED {
        let manifest = lemonfiber_plugin::Manifest::from_toml(pinned.copy);
        assert!(
            manifest.is_ok(),
            "{}: {manifest:?}",
            pinned.first_party.plugin
        );
        let Ok(manifest) = manifest else {
            continue;
        };
        for proof in manifest
            .proofs
            .iter()
            .filter(|proof| proof.request.is_some())
        {
            let service = manifest
                .services
                .iter()
                .find(|service| Some(&service.id) == proof.service.as_ref())
                .or_else(|| {
                    manifest
                        .services
                        .first()
                        .filter(|_| manifest.services.len() == 1)
                });
            let Some(service) = service.filter(|service| service.port.is_none()) else {
                continue;
            };
            assert!(
                service.listens.is_some(),
                "{}: {} publishes no port and listens on none",
                pinned.first_party.plugin,
                proof.id
            );
            asked_inside.push(proof.id.clone());
        }
    }
    for expected in ["gluetun.guarded", "unpackerr.metrics"] {
        assert!(
            asked_inside.iter().any(|id| id == expected),
            "{expected} is not among {asked_inside:?}"
        );
    }
}

#[tokio::test]
async fn a_build_whose_bundle_pins_no_fence_asks_nothing_and_says_so() {
    let program = Proving::exiting(0, ANSWERED);
    let ctx = machine(&[SOLVER], &program);
    let (request, expect) = (asking("GET", "/"), expecting(200));
    let asked = Asked {
        request: &request,
        expect: &expect,
        image: None,
    };

    let verdict = answering(&ctx, &solver(false), &asked, ctx.seams.clock.now()).await;

    assert!(
        matches!(&verdict, Verdict::Unproven { why } if why.contains("pins no fence")),
        "{verdict:?}"
    );
    assert!(program.asked().is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_service_not_answering_yet_is_asked_again_until_the_deadline() {
    let program = Proving::exiting(1, "");
    let ctx = crate::test_support::a_context()
        .engine(Arc::new(Reporting::holding(
            &[SOLVER],
            Lifecycle::Running,
            Health::Healthy,
        )))
        .clock(Arc::new(lemonfiber_fixtures::ports::Ticking::by(
            std::time::Duration::from_secs(1),
        )))
        .build()
        .with_program(Arc::clone(&program) as Arc<dyn Program>);
    let (request, expect) = (asking("GET", "/"), expecting(200));
    let asked = Asked {
        request: &request,
        expect: &expect,
        image: lemonfiber_bundled::FENCE,
    };
    let deadline = ctx.seams.clock.now() + std::time::Duration::from_secs(2);

    let verdict = answering(&ctx, &solver(false), &asked, deadline).await;

    assert!(matches!(verdict, Verdict::Unproven { .. }), "{verdict:?}");
    assert!(program.asked().len() >= 2, "{:?}", program.asked().len());
}

#[tokio::test]
async fn an_engine_that_will_not_say_what_runs_asks_nothing_and_says_so() {
    let program = Proving::exiting(0, ANSWERED);
    let ctx = crate::test_support::a_context()
        .engine(Arc::new(Reporting::absent()))
        .build()
        .with_program(Arc::clone(&program) as Arc<dyn Program>);

    let verdict = asked(&ctx, &solver(false), &asking("GET", "/")).await;

    assert!(
        matches!(&verdict, Verdict::Unproven { why } if why.contains("would not say where")),
        "{verdict:?}"
    );
    assert!(program.asked().is_empty());
}
