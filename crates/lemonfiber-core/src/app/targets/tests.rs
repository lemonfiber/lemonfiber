use super::{aggregator_target, project_directory};
use crate::app::targets::downloads::committed_of;
use crate::app::Ctx;
use crate::ports::filesystem::Beneath;
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

/// A service whose credential lives in `file`, confined to `within` where it is a
/// plugin's.
fn keyed_in(file: &str, within: Option<&str>) -> crate::wiring::Filler {
    crate::wiring::Filler {
        id: "stand-in".to_owned(),
        name: "Stand-in".to_owned(),
        origin: crate::origin::Origin::Bundled,
        address: None,
        adapter: None,
        published: None,
        key_file: Some(std::path::PathBuf::from(file)),
        confined_to: within.map(std::path::PathBuf::from),
        media_types: Vec::new(),
    }
}

/// A plugin's credential is read only from beneath the directory its container owns;
/// the stack's own is read where it is, and a service naming no file reads nothing.
#[tokio::test]
async fn a_plugins_credential_is_read_only_from_beneath_its_directory() {
    let files = lemonfiber_fixtures::files::Files::at(vec![
        (
            std::path::PathBuf::from("/stack/config/stand-in/key.ini"),
            "beneath",
        ),
        (std::path::PathBuf::from("/stack/secret"), "the host's own"),
    ]);
    let context = ctx().with_filesystem(files);
    let read = |filler: crate::wiring::Filler| {
        let context = &context;
        async move { super::credential_file(context, &filler).await }
    };

    assert_eq!(
        read(keyed_in(
            "/stack/config/stand-in/key.ini",
            Some("/stack/config/stand-in")
        ))
        .await,
        Beneath::Read("beneath".to_owned())
    );
    assert_eq!(
        read(keyed_in("/stack/secret", Some("/stack/config/stand-in"))).await,
        Beneath::Escaped
    );
    assert_eq!(
        read(keyed_in("/stack/secret", None)).await,
        Beneath::Read("the host's own".to_owned())
    );
    assert_eq!(read(keyed_in("/stack/absent", None)).await, Beneath::Absent);
    let mut unkeyed = keyed_in("/stack/secret", None);
    unkeyed.key_file = None;
    assert_eq!(read(unkeyed).await, Beneath::Absent);
}

/// Each of the three a confined read can come to, through one filesystem: a plain file
/// beneath is read, one resolving away is refused, and one that does not resolve at all
/// is absent rather than refused — nothing is there yet, which is the ordinary case of a
/// key not written.
///
/// One filesystem for all three on purpose: the coverage gate reads the read's best
/// instantiation alone, so the three have to be taken by the same one.
#[tokio::test]
async fn a_confined_read_comes_to_read_refused_or_absent() {
    let context = ctx().with_filesystem(std::sync::Arc::new(
        lemonfiber_fixtures::support::SeedFs::keyed(Some("held"), None)
            .missing(vec!["gone"])
            .leading_away(vec!["away"]),
    ));
    let read = |file: &'static str| {
        let context = &context;
        async move {
            super::credential_file(context, &keyed_in(file, Some("/stack/config/stand-in"))).await
        }
    };

    assert_eq!(
        read("/stack/config/stand-in/key.ini").await,
        Beneath::Read("held".to_owned())
    );
    assert_eq!(
        read("/stack/config/stand-in/away.ini").await,
        Beneath::Escaped
    );
    assert_eq!(
        read("/stack/config/stand-in/gone.ini").await,
        Beneath::Absent
    );
}

/// A refused credential file is said in the plugin's name where a plugin brought it,
/// and in the service's where it did not.
#[test]
fn a_refused_credential_file_is_said_in_the_name_of_whoever_brought_it() {
    let mut brought = keyed_in("/stack/secret", None);
    brought.origin = crate::origin::Origin::Plugin {
        named: "nzbget".to_owned(),
    };

    assert_eq!(
        super::escaped(&brought),
        "nzbget's credential file is a link, leads outside the directory its container \
         owns, or is not a file at all, so it was not read"
    );
    assert!(super::escaped(&keyed_in("/stack/secret", None)).starts_with("Stand-in's"));
}
