use crate::app::Ctx;
use crate::ports::docker::Lifecycle;
use crate::stack::compose::Action;

pub(crate) async fn running(ctx: &Ctx, services: &[String]) -> Vec<String> {
    if services.is_empty() {
        return Vec::new();
    }
    let Ok(containers) = ctx.seams.engine.list(&ctx.settings.project).await else {
        return Vec::new();
    };
    services
        .iter()
        .filter(|service| {
            containers.iter().any(|container| {
                container.service == **service && container.lifecycle == Lifecycle::Running
            })
        })
        .cloned()
        .collect()
}

pub(crate) async fn restarted(ctx: &Ctx, services: &[String]) -> Option<String> {
    if services.is_empty() {
        return None;
    }
    let action = Action::Start(services.to_vec());
    let invocation = match crate::app::engine::invocation(ctx, &[], &action) {
        Ok(invocation) => invocation,
        Err(problem) => return Some(problem.summary),
    };
    crate::app::engine::halted::before(ctx, &action, &invocation.addressed);
    let said = match ctx.seams.runner.run(&invocation.command).await {
        Ok(output) if output.succeeded() => None,
        Ok(refused) => Some(format!(
            "the container engine would not recreate them: {}",
            refused.stderr.trim()
        )),
        Err(why) => Some(why.to_string()),
    };
    crate::app::engine::halted::after(ctx, &action, &invocation.addressed).await;
    said
}
