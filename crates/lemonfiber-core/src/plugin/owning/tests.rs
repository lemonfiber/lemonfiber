//! Whose a journalled change is.

use super::{owner, owns};
use crate::journal::{Change, Kind, OPERATIONS};

fn under(operation: &str, kind: Kind) -> Change {
    Change {
        at: "1".to_owned(),
        operation: operation.to_owned(),
        target: "somewhere".to_owned(),
        kind,
    }
}

fn made() -> Kind {
    Kind::Made {
        path: "/stack/compose/plugins/komga.yml".to_owned(),
    }
}

fn region(marked: &str) -> Kind {
    Kind::Region {
        path: "/stack/config/caddy/Caddyfile".to_owned(),
        key: "proxy".to_owned(),
        owner: marked.to_owned(),
        written: 7,
    }
}

fn set() -> Kind {
    Kind::Set {
        key: "TZ".to_owned(),
        previous: None,
        current: "UTC".to_owned(),
    }
}

#[test]
fn a_plugins_region_is_named_as_the_plugins() {
    assert_eq!(owner("comics"), "plugin comics");
}

/// Every change under the plugin's own operation is its own, whatever it did.
#[test]
fn every_change_under_the_plugins_operation_is_the_plugins() {
    for kind in [made(), region("plugin komga"), set()] {
        assert!(owns("komga", &under("plugin komga", kind)));
    }
    assert!(!owns("kavita", &under("plugin komga", made())));
}

/// Under the bare id, a path made and a region marked as the plugin's are its own; a
/// setting, and a region somebody else's markers bound, are not.
#[test]
fn under_the_bare_id_only_what_an_install_writes_is_the_plugins() {
    assert!(owns("komga", &under("komga", made())));
    assert!(owns("komga", &under("komga", region("plugin komga"))));
    assert!(!owns("komga", &under("komga", region("plugin kavita"))));
    assert!(!owns("komga", &under("komga", set())));
    assert!(!owns("komga", &under("kavita", made())));
}

/// A plugin whose id is one of lemonfiber's operations owns nothing journalled under
/// that operation, and still owns what is journalled under its own.
#[test]
fn a_plugin_named_after_an_operation_of_ours_owns_none_of_its_changes() {
    for ours in OPERATIONS {
        for kind in [made(), region(&owner(ours)), set()] {
            assert!(!owns(ours, &under(ours, kind)), "{ours}");
        }
        assert!(owns(ours, &under(&owner(ours), made())), "{ours}");
    }
}

/// No operation of lemonfiber's own is spelled the way a plugin's is.
#[test]
fn no_operation_of_ours_is_spelled_as_a_plugins() {
    for ours in OPERATIONS {
        assert!(!ours.starts_with(&owner("")), "{ours}");
    }
}
