use crate::app::targets::downloads::committed_of;
use crate::app::Ctx;
use crate::ports::filesystem::Beneath;
use crate::ports::service::Download;
use crate::test_support::a_context;

/// A context over the real stack, for the resolution that reads only the manifest.
fn ctx() -> Ctx {
    a_context().build()
}

/// The embedded stack's fillers, with every part the bundle installs moved in where
/// `moved` says.
fn shipped(moved: bool) -> crate::wiring::Fillers {
    let installed: Vec<crate::plugin::Installed> = if moved {
        lemonfiber_bundled::PINNED
            .iter()
            .filter(|one| one.stage == Some(lemonfiber_bundled::Stage::Installed))
            .filter_map(|one| {
                crate::test_support::adapted::Adapted {
                    plugin: one.first_party.plugin,
                    adapter: "",
                    port: 0,
                }
                .installed()
            })
            .collect()
    } else {
        Vec::new()
    };
    crate::test_support::stack()
        .manifest()
        .map(|manifest| {
            crate::wiring::Fillers::of(
                &manifest,
                &installed,
                &crate::wiring::Chosen::default(),
                None,
            )
        })
        .unwrap_or_default()
}

/// Every service the shipped stack declares is one this build reaches once every part
/// the bundle installs has moved in.
#[test]
fn the_shipped_stack_declares_no_service_this_build_cannot_reach() {
    let found = super::unsupported_here(&shipped(true));

    assert!(
        found.is_empty(),
        "a service this build ships declares an API it cannot reach: {found:?}"
    );
}

/// Everything this build cannot cover reads in the order of the services it names, not
/// the order the stack declares them in.
///
/// Two declarations, declared the other way round from the order they are named in, so
/// the order is observable.
#[test]
fn what_this_build_cannot_cover_reads_in_the_order_of_the_services_it_names() {
    let declared = crate::test_support::stack()
        .manifest()
        .ok()
        .and_then(|manifest| manifest.services.into_iter().find(|one| one.id == "sonarr"));
    let theirs: Vec<lemonfiber_manifest::Service> = ["zeta", "alpha"]
        .into_iter()
        .filter_map(|id| {
            declared.clone().map(|mut service| {
                id.clone_into(&mut service.id);
                service
            })
        })
        .collect();
    assert_eq!(theirs.len(), 2, "the stack declares an episode filer");

    let found =
        super::unsupported_here(&crate::test_support::stack_fillers(theirs, &[], None, &[]));

    let named: Vec<&str> = found.iter().map(|report| report.what.as_str()).collect();
    assert_eq!(named, vec!["alpha", "zeta"], "{found:?}");
}

/// The stack's own indexer speaks no contract, so it is no indexer anything here asks.
#[tokio::test]
async fn an_indexer_speaking_no_contract_is_not_asked() {
    assert!(super::indexer_aggregator(&ctx(), &shipped(false))
        .await
        .is_none());
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
        minted: None,
        granted: Vec::new(),
        id: "stand-in".to_owned(),
        name: "Stand-in".to_owned(),
        origin: crate::origin::Origin::Bundled,
        address: None,
        adapter: None,
        published: None,
        key_file: Some(std::path::PathBuf::from(file)),
        confined_to: within.map(std::path::PathBuf::from),
        media_types: Vec::new(),
        provides: Vec::new(),
        contracts: Vec::new(),
        first_party: false,
        majors: Vec::new(),
        fronts: None,
        upstream: std::collections::BTreeMap::new(),
        native: None,
        replaces: None,
        via: None,
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
    // With no directory to hold it beneath — no stack on disk — nothing is read at all,
    // rather than a file read wherever it leads.
    assert_eq!(read(keyed_in("/stack/secret", None)).await, Beneath::Absent);
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
        lemonfiber_fixtures::support::SeedFs::keyed(Some("held"))
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
         owns, is not a file at all, or is too large to be one, so it was not read"
    );
    assert!(super::escaped(&keyed_in("/stack/secret", None)).starts_with("Stand-in's"));
}
