use crate::app::targets::downloads::committed_of;
use crate::app::Ctx;
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
