//! Resolving stack services to the targets that reading a credential starts
//! from — the project root a config is read under, and the Servarr-shape
//! services whose credential can be proven. Seeding and diagnosis both begin
//! here, so the resolution lives in one place they can share.
//!
//! Five questions, one per file: where things sit, which services speak the Servarr shape,
//! which download clients the stack has, what lemonfiber recorded for itself, and how to
//! open a client for any of them. Re-exported as one, so callers see the module they
//! always did.

mod downloads;
mod layout;
mod opening;
mod secrets;
mod servarr;

pub(super) use downloads::*;
pub(super) use layout::*;
pub(super) use opening::*;
pub(super) use secrets::*;
pub(super) use servarr::*;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{aggregator_target, project_directory};
    use crate::app::targets::downloads::committed_of;
    use crate::app::Ctx;
    use crate::config::Settings;
    use crate::platform::Environment;
    use crate::ports::service::Download;
    use crate::test_support::{spoke, stack, Reporting, Scripted};

    /// A context over the real stack, for the resolution that reads only the manifest.
    fn ctx() -> Ctx {
        Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::absent()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            Settings::default(),
            Environment::MacOs,
        )
    }

    /// Every service the embedded stack declares, with the project it is read from.
    fn resolving() -> (
        Vec<lemonfiber_manifest::Service>,
        Option<std::path::PathBuf>,
    ) {
        let context = ctx();
        let services = context
            .stack
            .checked_manifest(context.today())
            .map(|manifest| manifest.services)
            .unwrap_or_default();
        let project = project_directory(&context.stack, context.settings.stack_dir.as_deref());
        (services, project)
    }

    /// The walk carries on past a service that files no media and is no target of its
    /// own — the proxy solver in the middle of the list is exactly that, and stopping
    /// at it would leave the aggregator unresolved on a stack that has one.
    #[test]
    fn a_service_that_files_no_media_and_is_no_target_does_not_end_the_walk() {
        let (services, project) = resolving();
        let without_the_aggregator: Vec<lemonfiber_manifest::Service> = services
            .into_iter()
            .filter(|service| service.id != "prowlarr")
            .collect();
        assert!(aggregator_target(&without_the_aggregator, project.as_deref()).is_none());
    }

    /// A download with only its bytes-still-to-write set — the one field the
    /// committed sum reads.
    fn download(remaining: Option<u64>) -> Download {
        Download {
            name: "item".to_owned(),
            progress: 0,
            speed: None,
            eta: None,
            remaining,
        }
    }

    #[test]
    fn committed_sums_what_each_download_still_has_to_write() {
        let downloads = [download(Some(300)), download(Some(200))];
        assert_eq!(committed_of(&downloads), 500);
    }

    #[test]
    fn a_download_reporting_no_figure_is_left_out_of_the_sum() {
        // An unknown is not counted as zero-left; it is simply left out, so a
        // client that reports no figure never reads as "nothing more to write".
        let downloads = [download(Some(200)), download(None)];
        assert_eq!(committed_of(&downloads), 200);
    }

    #[test]
    fn the_sum_saturates_rather_than_wrapping() {
        let downloads = [download(Some(u64::MAX)), download(Some(1))];
        assert_eq!(committed_of(&downloads), u64::MAX);
    }
}
