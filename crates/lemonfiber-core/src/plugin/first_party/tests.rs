use super::{holds, FirstParty, EMBEDDED};
use crate::test_support::an_installed;

#[test]
fn a_plugin_is_first_party_only_by_id_and_manifest_alike() {
    let mut installed = an_installed("komga", Vec::new());
    installed.manifest = "ab12".to_owned();
    let set = [FirstParty {
        plugin: "komga",
        manifest: "ab12",
    }];

    assert!(holds(&set, &installed));
    installed.manifest = "cd34".to_owned();
    assert!(!holds(&set, &installed));
    assert!(!holds(EMBEDDED, &installed));
}
