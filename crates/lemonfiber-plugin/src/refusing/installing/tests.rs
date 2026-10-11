use crate::refusing::tests::{names, with, INSTALLABLE};
use crate::Manifest;

/// What an install or an update refuses of `text` beyond what reading it does.
pub(crate) fn uninstallable(text: &str) -> Vec<String> {
    Manifest::from_toml(text).map_or_else(
        |refused| vec![refused.to_string()],
        |manifest| {
            crate::uninstallable(&manifest)
                .iter()
                .map(ToString::to_string)
                .collect()
        },
    )
}

#[test]
fn a_service_naming_an_in_process_adapter_is_refused_at_install() {
    let refused = uninstallable(&with(
        "api = { kind = \"servarr\", key_source = \"config-xml\", path = \"/config/config.xml\", version = 3 }\nlistens = 8989\n",
    ));
    assert!(
        names(&refused, &["service komga.api", "in-process adapter"]),
        "{refused:?}"
    );
}

#[test]
fn a_plugin_with_neither_is_installable() {
    assert!(uninstallable(INSTALLABLE).is_empty());
}
