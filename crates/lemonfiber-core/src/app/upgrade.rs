//! Upgrading existing content to the chosen quality — a separate, cost-stated,
//! confirmed action.
//!
//! Changing a preset is forward-looking: it decides what is acquired next, never a
//! rewrite of what is on disk. Upgrading existing content is the opposite, and
//! costly — it re-acquires the library at the higher quality, potentially terabytes
//! of bandwidth and hours to days of time. So it is never a side effect of a preset
//! change: it is this explicit action, which states that cost and does nothing until
//! confirmed, and only then asks each \*arr to re-search what it already has for a
//! better release meeting the raised bar.

use super::targets::{project_directory, servarr_targets};
use super::Ctx;
use crate::doctor::credentials::Target;
use crate::error::{Diagnose, Problem};
use crate::model::{Triggered, UpgradeMedia, UpgradeReport};
use crate::ports::service::Maintenance;
use crate::recyclarr::Kind;

/// State the cost of upgrading existing content to the chosen quality, per media
/// type, and — only when confirmed — ask each resolution \*arr to re-search its
/// library for upgrades.
///
/// The cost is stated per media type because each carries its own preset: film at
/// maximum and television at space-saving are upgraded to different bars. Only the
/// resolution \*arrs present in the stack are covered; a music or index service is
/// not a resolution preset's concern, so it is left out rather than sent a command
/// it has no equivalent for.
pub(super) async fn upgrade(ctx: &Ctx, confirm: bool) -> Result<UpgradeReport, Box<Problem>> {
    let selection = super::quality::recorded_selection(ctx);
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());

    let mut media = Vec::new();
    for target in servarr_targets(&manifest.services, project.as_deref()) {
        let Some(kind) = Kind::for_section(&target.id) else {
            continue;
        };
        let preset = selection.for_type(kind.media_type());
        // Unconfirmed states the cost and touches nothing — the deliberate gate a
        // large, bandwidth-expensive operation sits behind.
        let outcome = if confirm {
            Some(trigger(ctx, kind, &target).await)
        } else {
            None
        };
        media.push(UpgradeMedia {
            media_type: kind.media_type().to_owned(),
            preset: preset.label().to_owned(),
            size_per_hour: preset.consequence().size_per_hour.to_owned(),
            outcome,
        });
    }
    Ok(UpgradeReport {
        confirmed: confirm,
        media,
    })
}

/// Ask one resolution \*arr to re-search its existing content for an upgrade, and read
/// what it said. The \*arr searches against its own current cutoff — whatever the last
/// applied preset set — so what actually upgrades is what sits below that bar.
async fn trigger(ctx: &Ctx, kind: Kind, target: &Target) -> Triggered {
    match target.open(&ctx.http, ctx.filesystem.as_ref()).await {
        None => Triggered::NotStarted,
        Some(service) => match service.run_command(kind.upgrade_command()).await {
            Ok(()) => Triggered::Started,
            Err(failure) => Triggered::Failed {
                detail: failure.to_string(),
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::upgrade;
    use super::Ctx;
    use crate::config::Settings;
    use crate::model::Triggered;
    use crate::platform::Environment;
    use crate::quality::Preset;
    use crate::stack::Source;
    use crate::test_support::{spoke, stack, Reporting, Scripted, SeedFs};
    use lemonfiber_fixtures::http::Fake;

    /// The Servarr config file `SeedFs` hands back for any \*arr, carrying a readable
    /// key so a target opens.
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";

    /// A context over the real stack (which names Sonarr and Radarr), the given
    /// filesystem, and HTTP that answers the upgrade POSTs from `replies`.
    fn ctx(fs: Arc<SeedFs>, replies: Vec<(u16, &'static str)>) -> Ctx {
        Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::absent()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            Settings::default(),
            Environment::MacOs,
        )
        .with_filesystem(fs)
        .with_http(Fake::scripted(replies))
    }

    fn outcomes(report: &crate::model::UpgradeReport) -> Vec<Option<&Triggered>> {
        report
            .media
            .iter()
            .map(|media| media.outcome.as_ref())
            .collect()
    }

    #[tokio::test]
    async fn unconfirmed_states_the_cost_per_media_and_triggers_nothing() {
        let report = upgrade(&ctx(Arc::new(SeedFs::keyed(None, None)), vec![]), false)
            .await
            .unwrap_or_default();
        assert!(!report.confirmed);
        // Both resolution media types, each with the default preset's cost stated and
        // no outcome — nothing was touched.
        assert_eq!(
            report.media.len(),
            2,
            "television and film are both covered"
        );
        assert!(report.media.iter().all(|media| {
            media.preset == Preset::default_preset().label()
                && media.size_per_hour.contains("GB")
                && media.outcome.is_none()
        }));
    }

    #[tokio::test]
    async fn a_confirmed_upgrade_starts_a_research_on_each_resolution_arr() {
        // Both media *arrs accept the command; music and index services are left out.
        let report = upgrade(
            &ctx(
                Arc::new(SeedFs::keyed(Some(KEYED), None)),
                vec![(201, ""), (201, "")],
            ),
            true,
        )
        .await
        .unwrap_or_default();
        assert!(report.confirmed);
        assert_eq!(report.media.len(), 2, "only Sonarr and Radarr are asked");
        assert!(outcomes(&report)
            .iter()
            .all(|outcome| matches!(outcome, Some(Triggered::Started))));
    }

    #[tokio::test]
    async fn a_per_type_choice_states_each_types_own_preset() {
        // The report speaks per media type: a maximum-for-film, space-saving-for-tv
        // split is not flattened to one figure.
        let env =
            std::env::temp_dir().join(format!("lemonfiber-upgrade-split-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&env);
        let env = env.join(".env");
        let mut selection = crate::quality::Selection::everywhere(Preset::Balanced);
        selection.set_type("movies", Preset::Maximum);
        selection.set_type("tv", Preset::SpaceSaving);
        let _ = crate::config::store::write(
            &env.with_file_name("quality.json"),
            &serde_json::to_string(&selection).unwrap_or_default(),
        );
        let mut context = ctx(Arc::new(SeedFs::keyed(None, None)), vec![]);
        context.settings.env_file = Some(env);

        let report = upgrade(&context, false).await.unwrap_or_default();
        let presets: Vec<&str> = report
            .media
            .iter()
            .map(|media| media.preset.as_str())
            .collect();
        assert!(presets.contains(&Preset::Maximum.label()));
        assert!(presets.contains(&Preset::SpaceSaving.label()));
    }

    #[tokio::test]
    async fn an_arr_not_yet_started_is_reported_not_started() {
        // No key on disk: the service has not finished starting, so it is not a fault.
        let report = upgrade(&ctx(Arc::new(SeedFs::keyed(None, None)), vec![]), true)
            .await
            .unwrap_or_default();
        assert_eq!(report.media.len(), 2);
        assert!(outcomes(&report)
            .iter()
            .all(|outcome| matches!(outcome, Some(Triggered::NotStarted))));
    }

    #[tokio::test]
    async fn an_arr_that_refuses_the_command_is_reported_failed() {
        let report = upgrade(
            &ctx(
                Arc::new(SeedFs::keyed(Some(KEYED), None)),
                vec![(500, "boom"), (500, "boom")],
            ),
            true,
        )
        .await
        .unwrap_or_default();
        assert!(outcomes(&report)
            .iter()
            .all(|outcome| matches!(outcome, Some(Triggered::Failed { .. }))));
    }

    #[tokio::test]
    async fn a_confirmed_upgrade_over_an_unreadable_stack_is_an_error() {
        let bad = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::absent()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            Source::External(std::path::Path::new("/lemonfiber/no/such/stack")),
            Settings::default(),
            Environment::MacOs,
        );
        assert!(upgrade(&bad, true).await.is_err());
    }
}
