//! The services the operator declared unmanaged, taken out of a pass and said.

/// Take the services the operator declared unmanaged out of the manifest, and say what
/// was taken, in the words they gave for taking it.
///
/// Removed at the top of a pass rather than gated at each of the dozen places that
/// would otherwise write to one. A gate per write point is a gate somebody adds a thirteenth
/// write beside, and the promise being kept — that lemonfiber observes and never
/// writes — is one a thirteenth write breaks silently.
///
/// Reported rather than simply absent, and reported as settled information rather than
/// as drift: a run that said nothing about a service the operator asked it to leave
/// alone would be indistinguishable from one that forgot the service existed.
pub(super) fn withheld(
    services: &mut Vec<lemonfiber_manifest::Service>,
    declared: &[(String, String)],
) -> Vec<crate::seed::Wiring> {
    let mut observed = Vec::new();
    services.retain(|service| {
        let Some(because) = crate::unmanaged::covering(declared, &service.id) else {
            return true;
        };
        observed.push(crate::seed::Wiring::settled(
            service.name.clone(),
            crate::seed::State::Observed {
                reason: because.to_owned(),
            },
        ));
        false
    });
    observed
}

/// The same, for the services installed plugins brought: each one the operator declared
/// unmanaged is taken out of its plugin's record for the pass, and said.
///
/// A plugin's service is one this pass can write to — a plugin's curator is told about
/// its download clients as the stack's is — so the promise is the same one, and kept the
/// same way.
pub(super) fn withheld_brought(
    installed: &[crate::plugin::Installed],
    declared: &[(String, String)],
) -> (Vec<crate::plugin::Installed>, Vec<crate::seed::Wiring>) {
    let mut observed = Vec::new();
    let kept = installed
        .iter()
        .map(|one| {
            let mut one = one.clone();
            one.services.retain(|placed| {
                let Some(because) = crate::unmanaged::covering(declared, &placed.service) else {
                    return true;
                };
                observed.push(crate::seed::Wiring::settled(
                    placed.called().to_owned(),
                    crate::seed::State::Observed {
                        reason: because.to_owned(),
                    },
                ));
                false
            });
            one
        })
        .collect();
    (kept, observed)
}

/// The stack's services and what is installed here, each less what the operator declared
/// unmanaged, and who fills each ask among them, as a pass reads them: nothing where the
/// stack or the record of what is installed cannot be read.
pub(crate) fn managed(
    ctx: &crate::app::Ctx,
) -> Option<(
    Vec<lemonfiber_manifest::Service>,
    Vec<crate::plugin::Installed>,
    crate::wiring::Fillers,
)> {
    let mut manifest = ctx.stack.checked_manifest(ctx.today()).ok()?;
    let _ = withheld(&mut manifest.services, &ctx.settings.unmanaged);
    let register = crate::app::plugins::read(ctx).ok()?;
    let (installed, _) = withheld_brought(register.installed(), &ctx.settings.unmanaged);
    let project =
        crate::app::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let fillers = crate::wiring::Fillers::of(
        &manifest,
        &installed,
        &crate::app::targets::chosen_fillers(ctx),
        project.as_deref(),
    )
    .numbered(crate::app::targets::ordinals(ctx));
    Some((manifest.services, installed, fillers))
}
