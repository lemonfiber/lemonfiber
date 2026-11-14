//! A plugin whose service is fenced, installed where the engine has not yet been asked
//! whether its containers hold a fence's rules or what its host's addresses are.

use lemonfiber_fixtures::proving::Proving;

use super::*;
use crate::ports::program::Program;

/// The proving manifest, its service fenced.
fn fenced() -> String {
    PROVING.replacen(
        "config_path = \"/app/data\"\n",
        "config_path = \"/app/data\"\nnetwork     = { fenced = true }\n",
        1,
    )
}

/// A machine the fenced plugin installs into, its engine running the fence as `program`
/// answers.
fn machine(name: &str, program: &Arc<Proving>) -> Ctx {
    proving(
        name,
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    )
    .with_program(Arc::clone(program) as Arc<dyn Program>)
}

#[tokio::test]
async fn an_install_asks_the_engines_host_its_addresses_and_writes_the_fence_beside_it() {
    let name = "fenced-installed";
    let program = Proving::commanded(
        vec![(
            crate::app::project::FENCE_PROBED,
            Proving::exited(0, crate::fence::HOLDS),
        )],
        Proving::exited(0, "203.0.113.7\n"),
    );
    let ctx = machine(name, &program);

    let outcome = installing(&ctx, &source(name, &fenced())).await;

    assert_eq!(counted(outcome), Some(1));
    let asked: Vec<Vec<String>> = program.asked().iter().map(|run| run.args.clone()).collect();
    assert!(
        asked.starts_with(&[
            vec![crate::app::project::FENCE_PROBED.to_owned()],
            vec![crate::app::project::FENCE_ASKED.to_owned()],
        ]),
        "{asked:?}"
    );
    let written = std::fs::read_to_string(stack_of(&ctx).join(crate::project::FILE));
    assert!(
        written
            .as_deref()
            .is_ok_and(|text| text.contains("komga-fence")
                && text.contains("network_mode: service:komga-fence")),
        "{written:?}"
    );
}

#[tokio::test]
async fn an_install_whose_fenced_service_cannot_be_fenced_is_refused_before_it_starts() {
    let name = "fenced-unaddressed";
    let ctx = machine(name, &Proving::unreachable());

    let outcome = installing(&ctx, &source(name, &fenced())).await;

    let detail = outcome
        .as_ref()
        .err()
        .and_then(|problem| problem.detail.clone())
        .unwrap_or_default();
    assert_eq!(
        refusal(outcome),
        crate::error::codes::stack::SERVICE_UNFENCED.to_string()
    );
    assert!(
        detail.contains("containers were not shown to hold") && detail.contains("was not asked"),
        "{detail}"
    );
    assert_eq!(counted(reading(&ctx).await), Some(0));
}

#[tokio::test]
async fn an_install_on_an_engine_whose_containers_cannot_hold_the_rules_is_refused_naming_why() {
    let name = "fenced-unheld";
    let program = Proving::exiting(
        1,
        "nft refused the ruleset: Error: Operation not permitted\n",
    );
    let ctx = machine(name, &program);

    let outcome = installing(&ctx, &source(name, &fenced())).await;

    let refused = outcome.as_ref().err();
    let detail = refused
        .and_then(|problem| problem.detail.clone())
        .unwrap_or_default();
    assert_eq!(
        refusal(outcome),
        crate::error::codes::stack::SERVICE_UNFENCED.to_string()
    );
    assert!(
        detail.contains("komga is fenced")
            && detail.contains("cannot hold the nftables rules a fence installs")
            && detail.contains("Operation not permitted"),
        "{detail}"
    );
    assert!(program
        .asked()
        .iter()
        .all(|run| run.args != [crate::app::project::FENCE_ASKED]));
    assert_eq!(counted(reading(&ctx).await), Some(0));
}
