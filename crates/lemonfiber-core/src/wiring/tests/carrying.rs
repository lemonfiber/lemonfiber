//! What a substitution leaves behind, read from what each contract declares held.

use super::{plugin_filling, written};
use crate::wiring::carrying::left_behind;

/// A curator replaced by one speaking the same contract leaves behind only what that
/// contract does not carry; replaced by one that does not speak it, every kind it holds
/// stays behind; and a part filling nothing a contract declares held leaves nothing.
#[test]
fn what_a_substitution_leaves_behind_is_read_from_the_contracts() {
    let speaking = |plugin: &str| {
        let mut one = plugin_filling(plugin, plugin, "library.curate");
        for placed in &mut one.services {
            placed.speaks = vec!["library.curate@1".to_owned()];
        }
        one
    };
    let installed = [
        speaking("sonarr"),
        speaking("medusa"),
        plugin_filling("sickgear", "sickgear", "library.curate"),
        plugin_filling("uptime", "uptime", "status.watch"),
    ];
    let manifest = written("");
    let behind = |capability: &str, was: &str, now: &str| -> Option<Vec<String>> {
        manifest.as_ref().map(|manifest| {
            left_behind(manifest, &installed, capability, was, now)
                .into_iter()
                .map(|one| one.kind)
                .collect()
        })
    };
    let every: Vec<String> = lemonfiber_contract::capabilities::library::curate::HOLDS
        .iter()
        .map(|held| held.kind.to_owned())
        .collect();

    assert_eq!(
        behind("library.curate", "sonarr", "medusa"),
        Some(vec![
            "quality-profiles".to_owned(),
            "download-history".to_owned()
        ])
    );
    assert_eq!(behind("library.curate", "sonarr", "sickgear"), Some(every));
    assert_eq!(
        behind("library.curate", "uptime", "medusa"),
        Some(Vec::new())
    );
    assert_eq!(
        behind("status.watch", "nowhere", "uptime"),
        Some(Vec::new())
    );
}
