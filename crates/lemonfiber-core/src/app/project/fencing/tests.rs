use std::path::PathBuf;
use std::sync::Arc;

use lemonfiber_fixtures::proving::Proving;
use lemonfiber_manifest::container::Network;

use crate::app::Ctx;
use crate::config::{Reaching, REACH_REGISTRY_KEY};
use crate::fence::{HELD, NFTABLES_HELD};
use crate::ports::program::{Ended, Joined, Program, Ran, Unran};

const IMAGE: &str =
    "ghcr.io/lemonfiber/fence:0.18.0@sha256:9e1d000000000000000000000000000000000000000000000000000000000000";

/// The stack's own service that runs fenced.
const FENCED_PART: &str = "flaresolverr";

/// A machine running the stack this build carries with a plugin's service installed
/// beside it, both fenced where `fenced` and neither otherwise, its engine running the
/// fence as `program` answers; answers where the project is.
fn machine(name: &str, fenced: bool, program: &Arc<Proving>) -> (Ctx, PathBuf) {
    let env = crate::test_support::env_without_password(name);
    let mut ctx = crate::test_support::a_context()
        .over(crate::test_support::embedded())
        .build()
        .with_program(Arc::clone(program) as Arc<dyn Program>);
    if !fenced {
        let fork = env.with_file_name("unfenced-stack");
        lemonfiber_fixtures::stack::manifest_without(&fork, FENCED_PART);
        ctx.stack = crate::stack::Source::External(Box::leak(fork.into_boxed_path()));
    }
    ctx.settings.stack_dir = Some(env.with_file_name("data").join("stack"));
    ctx.settings.env_file = Some(env);
    let mut service = crate::test_support::a_placed("solver", &[], None);
    service.fields.network = fenced.then_some(Network {
        via: None,
        fenced: true,
    });
    let mut register = crate::plugin::Register::empty();
    assert!(register
        .record(crate::test_support::an_installed("solver", vec![service]))
        .is_ok());
    assert!(
        crate::app::record::keep(crate::app::plugins::kept_at(&ctx).as_deref(), &register).is_ok()
    );
    let project = super::super::directory(&ctx).unwrap_or_default();
    (ctx, project)
}

/// An engine whose containers hold the rules a fence installs, answering every other run
/// with `rest`.
fn holding(rest: Result<Ran, Unran>) -> Arc<Proving> {
    Proving::commanded(vec![(super::PROBED, Proving::exited(0, "held\n"))], rest)
}

async fn readied(ctx: &Ctx, image: Option<&str>) -> Option<String> {
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok()?;
    super::readied(ctx, &manifest.services, &installed(ctx), image).await
}

/// What `ctx`'s register holds.
fn installed(ctx: &Ctx) -> Vec<crate::plugin::Installed> {
    crate::app::plugins::read(ctx)
        .map(|register| register.installed().to_vec())
        .unwrap_or_default()
}

#[tokio::test]
async fn the_engine_is_asked_whether_its_containers_hold_the_rules_before_its_addresses() {
    let program = holding(Proving::exited(0, "192.168.1.20\n"));
    let (ctx, project) = machine("fence-probed", true, &program);

    assert_eq!(readied(&ctx, Some(IMAGE)).await, None);

    assert_eq!(
        std::fs::read_to_string(project.join(NFTABLES_HELD))
            .ok()
            .as_deref(),
        Some("held")
    );
    assert!(super::super::nftables_held(&ctx));
    let asked = program.asked();
    let args: Vec<Vec<String>> = asked.iter().map(|run| run.args.clone()).collect();
    assert_eq!(
        args,
        [
            vec![super::PROBED.to_owned()],
            vec![super::ASKED.to_owned()]
        ]
    );
    let Some(run) = asked.first() else {
        return;
    };
    assert_eq!(run.image, IMAGE);
    assert_eq!(run.network, Joined::Nothing);
    assert_eq!(run.cap_drop, vec!["ALL".to_owned()]);
    assert_eq!(run.cap_add, vec!["NET_ADMIN".to_owned()]);
    assert!(run.read_only);
    assert_eq!(run.bound, None);
    assert!(run.env.is_empty());
    assert_eq!(run.user, None);
    assert_eq!(run.security_opt, vec!["no-new-privileges:true".to_owned()]);
}

#[tokio::test]
async fn the_engines_host_is_asked_its_addresses_holding_no_capability_and_the_answer_kept() {
    let program = holding(Proving::exited(0, "192.168.1.20\n2001:db8::1\n"));
    let (ctx, project) = machine("fence-asked", true, &program);

    assert_eq!(readied(&ctx, Some(IMAGE)).await, None);

    assert_eq!(
        std::fs::read_to_string(project.join(HELD)).ok().as_deref(),
        Some("192.168.1.20\n2001:db8::1\n")
    );
    assert_eq!(
        super::super::host_addresses(&ctx),
        crate::fence::kept("192.168.1.20\n2001:db8::1")
    );
    let asked = program.asked();
    assert_eq!(asked.len(), 2, "{asked:?}");
    let Some(run) = asked.get(1) else {
        return;
    };
    assert_eq!(run.image, IMAGE);
    assert_eq!(run.args, vec![super::ASKED.to_owned()]);
    assert_eq!(run.network, Joined::Host);
    assert_eq!(run.cap_drop, vec!["ALL".to_owned()]);
    assert!(run.cap_add.is_empty());
    assert!(run.read_only);
    assert_eq!(run.bound, None);
    assert_eq!(run.security_opt, vec!["no-new-privileges:true".to_owned()]);
}

#[tokio::test]
async fn containers_that_cannot_hold_the_rules_are_asked_no_address_and_say_why() {
    for (index, (program, said)) in [
        (
            Proving::exiting(
                1,
                "nft refused the ruleset: Error: Operation not permitted\n",
            ),
            "the engine's containers cannot hold the nftables rules a fence installs: nft \
             refused the ruleset: Error: Operation not permitted",
        ),
        (
            Proving::exiting(0, "192.168.1.20\n"),
            "answered \"192.168.1.20\" rather than \"held\"",
        ),
        (Proving::exiting(2, ""), "exited 2 rather than answering"),
        (Proving::overrunning("held\n"), "within 60 seconds"),
        (
            Proving::unreachable(),
            "was not asked whether its containers can hold nftables rules",
        ),
        (
            Proving::exiting(0, &"held\n".repeat(60_000)),
            "more than the",
        ),
        (
            Proving::answering(Ok(Ran {
                ended: Ended::Exited(0),
                stdout: vec![0xff, 0xfe],
            })),
            "not text",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let (ctx, project) = machine(&format!("fence-unheld-{index}"), true, &program);
        let _ = std::fs::create_dir_all(&project);
        let _ = std::fs::write(project.join(NFTABLES_HELD), "held\n");
        let _ = std::fs::write(project.join(HELD), "10.0.0.1\n");

        let why = readied(&ctx, Some(IMAGE)).await.unwrap_or_default();

        assert!(why.contains(said), "{said}: {why:?}");
        assert!(!super::super::nftables_held(&ctx), "{said}");
        assert!(super::super::host_addresses(&ctx).is_empty(), "{said}");
        let asked: Vec<Vec<String>> = program.asked().iter().map(|run| run.args.clone()).collect();
        assert_eq!(asked, [vec![super::PROBED.to_owned()]], "{said}");
    }
}

#[tokio::test]
async fn an_address_the_host_took_since_the_last_start_is_the_one_the_next_start_keeps() {
    let before = holding(Proving::exited(0, "192.168.1.20\n"));
    let (ctx, project) = machine("fence-moved", true, &before);
    assert_eq!(readied(&ctx, Some(IMAGE)).await, None);

    let after = holding(Proving::exited(0, "192.168.1.57\n"));
    let moved = ctx
        .clone()
        .with_program(Arc::clone(&after) as Arc<dyn Program>);
    assert_eq!(readied(&moved, Some(IMAGE)).await, None);

    assert_eq!(
        std::fs::read_to_string(project.join(HELD)).ok().as_deref(),
        Some("192.168.1.57\n")
    );
    assert_eq!(after.asked().len(), 2);
}

#[tokio::test]
async fn an_answer_that_cannot_be_trusted_whole_keeps_no_address_and_says_why() {
    for (index, (answer, said)) in [
        (Proving::exited(0, ""), "no address"),
        (
            Proving::exited(0, "192.168.1.20\nnot an address\n"),
            "no address",
        ),
        (Proving::exited(1, "192.168.1.20\n"), "exited 1"),
        (
            Ok(Ran {
                ended: Ended::Overran,
                stdout: b"192.168.1.20\n".to_vec(),
            }),
            "within 60 seconds",
        ),
        (
            Err(Unran::Unreachable {
                reason: "gone".to_owned(),
            }),
            "was not asked its addresses",
        ),
        (
            Proving::exited(0, &"192.168.1.20\n".repeat(30_000)),
            "more than the",
        ),
        (
            Ok(Ran {
                ended: Ended::Exited(0),
                stdout: vec![0xff, 0xfe],
            }),
            "not text",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let program = holding(answer);
        let (ctx, project) = machine(&format!("fence-untrusted-{index}"), true, &program);
        let _ = std::fs::create_dir_all(&project);
        let _ = std::fs::write(project.join(HELD), "10.0.0.1\n");

        let why = readied(&ctx, Some(IMAGE)).await.unwrap_or_default();

        assert!(why.contains(said), "{said}: {why:?}");
        assert_eq!(
            std::fs::read_to_string(project.join(HELD)).ok().as_deref(),
            Some(""),
            "{said}"
        );
        assert!(super::super::host_addresses(&ctx).is_empty());
    }
}

#[tokio::test]
async fn nothing_is_asked_without_a_fenced_service_a_pinned_fence_or_where_a_rehearsal_runs() {
    let program = Proving::exiting(0, "192.168.1.20\n");
    let (unfenced, _) = machine("fence-none", false, &program);
    assert_eq!(readied(&unfenced, Some(IMAGE)).await, None);
    let (unpinned, _) = machine("fence-unpinned", true, &program);
    assert_eq!(readied(&unpinned, None).await, None);
    let (mut rehearsed, _) = machine("fence-rehearsed", true, &program);
    rehearsed.dry_run = true;
    assert_eq!(readied(&rehearsed, Some(IMAGE)).await, None);
    let (mut stackless, _) = machine("fence-stackless", true, &program);
    stackless.settings.stack_dir = None;
    assert_eq!(readied(&stackless, Some(IMAGE)).await, None);
    assert!(program.asked().is_empty());
}

#[tokio::test]
async fn an_answer_that_cannot_be_kept_says_why() {
    for (index, file) in [HELD, NFTABLES_HELD].into_iter().enumerate() {
        let program = holding(Proving::exited(0, "192.168.1.20\n"));
        let (ctx, project) = machine(&format!("fence-unkept-{index}"), true, &program);
        assert!(std::fs::create_dir_all(project.join(file)).is_ok());

        assert!(readied(&ctx, Some(IMAGE)).await.is_some(), "{file}");
        assert_eq!(program.asked().len(), 2, "{file}");
    }
}

#[tokio::test]
async fn the_fences_image_is_pulled_only_where_the_registry_may_be_reached() {
    let program = holding(Proving::exited(0, "192.168.1.20\n"));
    let (mut ctx, _) = machine("fence-pulled", true, &program);
    assert_eq!(readied(&ctx, Some(IMAGE)).await, None);
    ctx.settings.reaching = Reaching::without(REACH_REGISTRY_KEY);
    assert_eq!(readied(&ctx, Some(IMAGE)).await, None);
    let pulled: Vec<bool> = program.asked().iter().map(|run| run.pull).collect();
    assert_eq!(pulled, vec![true, true, false, false]);
}
