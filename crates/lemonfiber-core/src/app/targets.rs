//! Resolving stack services to the targets that reading a credential starts
//! from — the project root a config is read under, and the Servarr-shape
//! services whose credential can be proven. Seeding and diagnosis both begin
//! here, so the resolution lives in one place they can share.
//!
//! Five questions, one per file: where things sit, which services speak the Servarr shape,
//! which download clients the stack has, what lemonfiber recorded for itself, and how to
//! open a client for any of them. Re-exported as one, so callers see the module they
//! always did.
//!
//! And one answer assembled from two of them: what in this stack a feature that has to
//! know what a service *is* cannot cover. Three reports carry it — the status reading,
//! a seed pass and a queue-health reading — and assembling it here is what keeps those
//! three from each deciding it differently.

mod downloads;
mod layout;
mod opening;
mod secrets;
mod servarr;

/// Everything in this stack that a feature needing to know what a service is cannot
/// cover, each with why, in one settled order.
///
/// Two sources and no third: a shape this build does not speak at all, and a Servarr
/// declaration this build cannot reach through. A service declaring no API is in
/// neither — the manifest saying nothing is the stack's own statement that there is
/// nothing to integrate with, and repeating it back is not information.
///
/// Sorted by the service it names, so a report reads the same twice and two reports
/// carrying it agree line for line. The two sources cannot name one service between
/// them — a shape is either spoken or not — so nothing is reported twice.
pub(crate) fn unsupported_here(
    services: &[lemonfiber_manifest::Service],
    project: Option<&std::path::Path>,
) -> Vec<crate::model::UnsupportedReport> {
    let mut found = crate::unsupported::deferred(services);
    found.extend(servarr::unreachable_targets(services, project));
    found.sort_by(|one, two| one.what.cmp(&two.what));
    found
}

pub(crate) use downloads::*;
pub(crate) use layout::*;
pub(crate) use opening::*;
pub(crate) use secrets::*;
pub(crate) use servarr::*;

#[cfg(test)]
mod tests {

    use super::{aggregator_target, project_directory};
    use crate::app::targets::downloads::committed_of;
    use crate::app::Ctx;
    use crate::ports::service::Download;
    use crate::test_support::a_context;

    /// A context over the real stack, for the resolution that reads only the manifest.
    fn ctx() -> Ctx {
        a_context().build()
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

    /// The shipped stack's only unsupported service is the one deferred on purpose.
    ///
    /// Not "none", which is what this claimed before somebody ran it: the stack ships
    /// the book indexer, whose shape is the single entry in
    /// [`crate::unsupported::DEFERRED`], so the honest assertion is that the list
    /// holds exactly that and nothing beside it. Written against the declaration
    /// rather than against a name spelled again here — a shape that stops being
    /// deferred stops being expected by the same edit that makes it speakable, and a
    /// second service arriving unsupported fails this whichever shape it declares.
    #[test]
    fn the_shipped_stack_defers_one_service_and_only_the_one() {
        let (services, project) = resolving();
        let found = super::unsupported_here(&services, project.as_deref());

        let deferred: Vec<&str> = crate::unsupported::DEFERRED
            .iter()
            .map(|(_, because)| *because)
            .collect();
        let unexpected: Vec<&crate::model::UnsupportedReport> = found
            .iter()
            .filter(|report| !deferred.contains(&report.because.as_str()))
            .collect();

        assert!(
            unexpected.is_empty(),
            "a service this build ships declares an API it cannot speak or reach, and \
             it is not one of the shapes deferred on purpose: {unexpected:?}"
        );
        assert_eq!(
            found.len(),
            1,
            "the stack ships exactly one deferred service — the book indexer: {found:?}"
        );
    }

    /// And a fork's incomplete declaration lands in the same list the deferred shapes
    /// do, sorted together rather than appended in whichever order they were found.
    #[test]
    fn a_declaration_this_build_cannot_reach_is_named_in_the_same_list() {
        let (services, project) = resolving();
        let mut theirs: Vec<lemonfiber_manifest::Service> = services
            .into_iter()
            .filter(|service| service.id == "sonarr")
            .collect();
        for service in &mut theirs {
            service.port = None;
        }
        assert_eq!(theirs.len(), 1, "the stack declares an episode filer");

        let found = super::unsupported_here(&theirs, project.as_deref());
        assert!(
            found.first().is_some_and(|one| one.what == "sonarr"),
            "{found:?}"
        );
    }

    /// Everything this build cannot cover reads in one order, whichever of the two
    /// sources named each of them.
    ///
    /// The list is assembled by appending one source to the other, so left alone it
    /// would carry the seam it was built along: the shapes this build does not speak
    /// first, then the declarations it cannot reach, each in manifest order. An
    /// operator reading it has no idea there are two sources and no reason to learn —
    /// what they have is a list of services with something wrong, and two of them next
    /// to each other should be next to each other for a reason they can see.
    ///
    /// Driven with two unreachable declarations rather than one, because a single
    /// entry proves nothing about an order and a pair already in order proves nothing
    /// either: the two chosen here are declared the other way round.
    #[test]
    fn what_this_build_cannot_cover_reads_in_one_order_whichever_source_named_it() {
        let (services, project) = resolving();
        // Two Servarr declarations with the port taken away — unreachable, and so in
        // the list beside the one shape this build defers on purpose.
        let theirs: Vec<lemonfiber_manifest::Service> = services
            .into_iter()
            .map(|mut service| {
                if service.id == "prowlarr" || service.id == "lidarr" {
                    service.port = None;
                }
                service
            })
            .collect();
        let declared: Vec<&str> = theirs
            .iter()
            .map(|service| service.id.as_str())
            .filter(|id| *id == "prowlarr" || *id == "lidarr")
            .collect();
        assert_eq!(
            declared,
            vec!["prowlarr", "lidarr"],
            "the stack declares these two in this order, which is what makes the sort \
             below observable"
        );

        let found = super::unsupported_here(&theirs, project.as_deref());

        let named: Vec<&str> = found.iter().map(|report| report.what.as_str()).collect();
        assert_eq!(named, vec!["bindery", "lidarr", "prowlarr"], "{found:?}");
    }

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
