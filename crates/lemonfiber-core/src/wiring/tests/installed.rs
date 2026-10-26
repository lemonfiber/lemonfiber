//! The shipped stack's asks with every part the bundle installs moved into its plugin.

use super::stack;
use crate::wiring::{filled, settle, Chosen, Reaches, Settled};

/// The stack's asks settled with every part the bundle installs moved into its plugin.
fn moved() -> Vec<crate::wiring::Wired> {
    stack()
        .map(|manifest| {
            settle(
                &manifest,
                &crate::test_support::adapted::installed_parts(),
                &Chosen::default(),
            )
        })
        .unwrap_or_default()
}

/// An ask for all of them is not a contest. Four services curate a library here,
/// and a reader that could only answer "one or refused" would refuse every link
/// in the stack that reaches all of them.
#[test]
fn an_ask_for_every_filler_reaches_every_filler() {
    let curators = ["bindery", "lidarr", "radarr", "sonarr"];
    let reaches = moved()
        .into_iter()
        .filter(|one| one.by == "bazarr")
        .map(|one| one.reaches)
        .find(|reaches| {
            matches!(
                reaches,
                Reaches::Asked {
                    settled: Settled::Each,
                    ..
                }
            )
        });
    assert_eq!(
        reaches,
        Some(Reaches::Asked {
            native: None,
            capability: "library.curate".to_owned(),
            services: curators.map(|one| format!("{one}-adapter")).to_vec(),
            settled: Settled::Each,
            origins: curators
                .iter()
                .map(|one| {
                    (
                        format!("{one}-adapter"),
                        crate::origin::Origin::Plugin {
                            named: (*one).to_owned(),
                        },
                    )
                })
                .collect(),
        })
    );
}

/// One capability resolves the same way for everything that asks for it, so what
/// is settled is one map rather than one answer per asker.
#[test]
fn what_fills_each_capability_is_one_answer_for_every_asker() {
    let held = filled(&moved());
    assert_eq!(
        held.get("download.torrent").map(Vec::as_slice),
        Some(["qbittorrent-adapter".to_owned()].as_slice())
    );
    assert_eq!(
        held.get("identity.source").map(Vec::as_slice),
        Some(["jellyfin-adapter".to_owned()].as_slice())
    );
    assert_eq!(
        held.get("indexer.search").map(Vec::as_slice),
        Some(["prowlarr-adapter".to_owned()].as_slice())
    );
}
