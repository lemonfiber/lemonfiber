use std::path::PathBuf;
use std::sync::Arc;

use lemonfiber_fixtures::proving::Proving;
use lemonfiber_manifest::container::{Shape, RENDER_DEVICE};

use crate::app::Ctx;
use crate::fence::RENDER_HELD;
use crate::ports::program::{Bound, Joined, Program};

const IMAGE: &str =
    "ghcr.io/lemonfiber/fence:0.18.0@sha256:9e1d000000000000000000000000000000000000000000000000000000000000";

/// A machine running the stack this build carries with a plugin's service installed
/// beside it, taking `render` where `renders`, its engine running the fence as `program`
/// answers; answers where the project is.
fn machine(name: &str, renders: bool, program: &Arc<Proving>) -> (Ctx, PathBuf) {
    let env = crate::test_support::env_without_password(name);
    let mut ctx = crate::test_support::a_context()
        .over(crate::test_support::embedded())
        .build()
        .with_program(Arc::clone(program) as Arc<dyn Program>);
    ctx.settings.stack_dir = Some(env.with_file_name("data").join("stack"));
    ctx.settings.env_file = Some(env);
    let mut service = crate::test_support::a_placed("viewer", &[], None);
    service.shape = renders.then_some(Shape::Render);
    let mut register = crate::plugin::Register::empty();
    assert!(register
        .record(crate::test_support::an_installed("viewer", vec![service]))
        .is_ok());
    assert!(
        crate::app::record::keep(crate::app::plugins::kept_at(&ctx).as_deref(), &register).is_ok()
    );
    let project = super::super::directory(&ctx).unwrap_or_default();
    (ctx, project)
}

/// The stack `ctx` runs, every service of it taking `render` taking none.
fn unrendering(ctx: &Ctx) -> Option<lemonfiber_manifest::Manifest> {
    let mut manifest = ctx.stack.checked_manifest(ctx.today()).ok()?;
    for service in &mut manifest.services {
        service.shape = service.shape.filter(|shape| *shape != Shape::Render);
    }
    Some(manifest)
}

async fn readied(ctx: &Ctx, image: Option<&str>) -> Option<String> {
    super::readied(ctx, &unrendering(ctx)?.services, &installed(ctx), image).await
}

/// What `ctx`'s register holds.
fn installed(ctx: &Ctx) -> Vec<crate::plugin::Installed> {
    crate::app::plugins::read(ctx)
        .map(|register| register.installed().to_vec())
        .unwrap_or_default()
}

#[tokio::test]
async fn the_engines_host_is_asked_its_render_group_holding_its_devices_read_only_alone() {
    let program = Proving::exiting(0, "44\n");
    let (ctx, project) = machine("render-asked", true, &program);

    assert_eq!(readied(&ctx, Some(IMAGE)).await, None);

    assert_eq!(
        std::fs::read_to_string(project.join(RENDER_HELD))
            .ok()
            .as_deref(),
        Some("44")
    );
    assert_eq!(super::super::render_group(&ctx), Some(44));
    let asked = program.asked();
    assert_eq!(asked.len(), 1, "{asked:?}");
    let Some(run) = asked.first() else {
        return;
    };
    assert_eq!(run.image, IMAGE);
    assert_eq!(run.args, vec![super::ASKED.to_owned()]);
    assert_eq!(run.network, Joined::Nothing);
    assert_eq!(run.cap_drop, vec!["ALL".to_owned()]);
    assert!(run.cap_add.is_empty());
    assert!(run.read_only);
    assert_eq!(
        run.bound,
        Some(Bound {
            source: RENDER_DEVICE.into(),
            target: RENDER_DEVICE.to_owned(),
            read_only: true,
        })
    );
    assert_eq!(run.security_opt, vec!["no-new-privileges:true".to_owned()]);
}

#[tokio::test]
async fn a_host_holding_no_render_device_keeps_no_group_and_is_no_fault() {
    let program = Proving::exiting(0, "");
    let (ctx, project) = machine("render-none", true, &program);
    let _ = std::fs::create_dir_all(&project);
    let _ = std::fs::write(project.join(RENDER_HELD), "44");

    assert_eq!(readied(&ctx, Some(IMAGE)).await, None);

    assert_eq!(
        std::fs::read_to_string(project.join(RENDER_HELD))
            .ok()
            .as_deref(),
        Some("")
    );
    assert_eq!(super::super::render_group(&ctx), None);
}

#[tokio::test]
async fn an_answer_that_is_no_group_or_no_answer_keeps_no_group_and_says_why() {
    for (index, (program, image, said)) in [
        (Proving::exiting(0, "render\n"), Some(IMAGE), "not one"),
        (Proving::exiting(0, "-44\n"), Some(IMAGE), "not one"),
        (Proving::exiting(1, "44\n"), Some(IMAGE), "exited 1"),
        (
            Proving::unreachable(),
            Some(IMAGE),
            "was not asked its render group",
        ),
        (Proving::exiting(0, "44\n"), None, "pins no fence"),
    ]
    .into_iter()
    .enumerate()
    {
        let (ctx, project) = machine(&format!("render-untrusted-{index}"), true, &program);
        let _ = std::fs::create_dir_all(&project);
        let _ = std::fs::write(project.join(RENDER_HELD), "44");

        let why = readied(&ctx, image).await.unwrap_or_default();

        assert!(why.contains(said), "{said}: {why:?}");
        assert_eq!(
            std::fs::read_to_string(project.join(RENDER_HELD))
                .ok()
                .as_deref(),
            Some(""),
            "{said}"
        );
        assert_eq!(super::super::render_group(&ctx), None, "{said}");
    }
}

#[tokio::test]
async fn nothing_is_asked_without_a_service_taking_render_or_where_a_rehearsal_runs() {
    let program = Proving::exiting(0, "44\n");
    let (unrendered, _) = machine("render-untaken", false, &program);
    assert_eq!(readied(&unrendered, Some(IMAGE)).await, None);
    let (mut rehearsed, _) = machine("render-rehearsed", true, &program);
    rehearsed.dry_run = true;
    assert_eq!(readied(&rehearsed, Some(IMAGE)).await, None);
    let (mut stackless, _) = machine("render-stackless", true, &program);
    stackless.settings.stack_dir = None;
    assert_eq!(readied(&stackless, Some(IMAGE)).await, None);
    assert!(program.asked().is_empty());
}

#[tokio::test]
async fn a_stack_service_taking_render_is_asked_for_too() {
    let program = Proving::exiting(0, "44\n");
    let (ctx, _) = machine("render-stacked", false, &program);
    let Some(mut manifest) = unrendering(&ctx) else {
        return;
    };
    let Some(service) = manifest.services.first_mut() else {
        return;
    };
    service.shape = Some(Shape::Render);

    assert_eq!(
        super::readied(&ctx, &manifest.services, &[], Some(IMAGE)).await,
        None
    );
    assert_eq!(program.asked().len(), 1);
}

#[tokio::test]
async fn an_answer_that_cannot_be_kept_says_why() {
    let program = Proving::exiting(0, "44\n");
    let (ctx, project) = machine("render-unkept", true, &program);
    assert!(std::fs::create_dir_all(project.join(RENDER_HELD)).is_ok());

    assert!(readied(&ctx, Some(IMAGE)).await.is_some());
}

#[test]
fn each_service_written_without_its_render_devices_is_said_to_run_without_hardware_transcoding() {
    let project = crate::project::Project {
        text: String::new(),
        topology: crate::topology::Topology::default(),
        unfenced: Vec::new(),
        unrendered: vec!["jellyfin".to_owned()],
    };
    assert_eq!(
        super::unrendered(&project),
        vec!["jellyfin runs without hardware transcoding".to_owned()]
    );
}
