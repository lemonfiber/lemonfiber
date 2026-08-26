//! Whether this machine may ask a registry for an image, and what it is told when
//! it may not.
//!
//! A setting that only turns something off in one of the two places it happens is a
//! setting that means nothing, so both are here: a fetch asked for on its own is
//! refused outright, and a start — which fetches whatever is missing before it
//! starts anything — is told never to pull, which is [`crate::stack::compose`]'s
//! half of the same decision.

use crate::app::Ctx;
use crate::config::REACH_REGISTRY_KEY;
use crate::error::{Amiss, Problem, Remedy, Severity};
use crate::stack::compose::Action;

/// Whether this action would ask a registry for anything the operator has refused.
pub(super) fn refused(ctx: &Ctx, action: &Action) -> bool {
    matches!(action, Action::Pull) && !ctx.settings.reaching.allows(REACH_REGISTRY_KEY)
}

/// What a fetch is told when this operator has switched fetching off.
///
/// A refusal rather than a run that quietly does nothing: asking to fetch and being
/// told the fetch happened, when nothing was fetched, is worse than being stopped.
/// The setting is named in both halves, so the way out is on the screen rather than
/// in a document.
pub(super) fn refusal() -> Problem {
    Problem::new(
        crate::app::REGISTRY_REFUSED,
        Severity::Error,
        "fetching images is switched off",
        format!(
            "Nothing was fetched. {REACH_REGISTRY_KEY} is off, so this machine asks no \
             registry for anything — a start uses the images already here, and a service \
             whose image is missing will not start."
        ),
        Remedy::new(format!(
            "Turn fetching back on with `lemonfiber config set {REACH_REGISTRY_KEY} on`, then \
             run this again"
        )),
    )
    .lies_in(Amiss::Asking)
}

#[cfg(test)]
mod tests {
    use super::{refusal, refused};
    use crate::config::{Reaching, Settings, REACH_REGISTRY_KEY};
    use crate::stack::compose::Action;
    use crate::test_support::a_context;

    /// A machine whose operator has switched fetching off and left the rest alone.
    fn refusing() -> crate::app::Ctx {
        a_context()
            .settings(Settings {
                reaching: Reaching::without(REACH_REGISTRY_KEY),
                ..Settings::default()
            })
            .build()
    }

    #[test]
    fn a_fetch_is_refused_only_where_the_operator_switched_fetching_off() {
        let allowed = a_context().build();
        assert!(!refused(&allowed, &Action::Pull));

        assert!(refused(&refusing(), &Action::Pull));
    }

    /// Only the fetch. A start still runs — with `--pull never`, which is the other
    /// half — because refusing to start a stack whose images are already here would
    /// be a setting about the network taking the machine offline.
    #[test]
    fn nothing_but_a_fetch_is_refused() {
        let refusing = refusing();
        for action in [
            Action::Up,
            Action::Start(Vec::new()),
            Action::Down,
            Action::Stop(Vec::new()),
            Action::Restart(Vec::new()),
            Action::Config,
        ] {
            assert!(!refused(&refusing, &action), "{action:?} was refused");
        }
    }

    #[test]
    fn the_refusal_names_the_setting_and_the_way_back() {
        let problem = refusal();
        assert!(problem.meaning.contains(REACH_REGISTRY_KEY), "{problem:?}");
        let offered: Vec<&str> = problem
            .remedies
            .iter()
            .map(|remedy| remedy.action.as_str())
            .collect();
        assert!(
            offered
                .iter()
                .any(|action| action.contains("config set") && action.contains(REACH_REGISTRY_KEY)),
            "{offered:?}"
        );
    }
}
