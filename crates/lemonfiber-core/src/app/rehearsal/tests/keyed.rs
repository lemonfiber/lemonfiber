//! What a rehearsal of asking the keys and the back-ends for something comes to.

use super::super::{asked, Rehearsal};
use crate::app::Command;

/// Asking the keys rehearses as each request is: a listing reads, a revoke says which
/// key it would refuse, and a mint cannot, because a key is its secret.
#[test]
fn asking_the_keys_rehearses_as_each_request_is() {
    use crate::keys::run::Asked as Keyed;
    use crate::keys::Minter;
    let list = asked(&Command::Keys(Keyed::List {
        by: Minter::Operator,
    }));
    assert!(matches!(list.rehearsal, Rehearsal::Reads));
    let revoke = asked(&Command::Keys(Keyed::Revoke {
        name: "ha".to_owned(),
        by: Minter::Operator,
    }));
    assert!(matches!(revoke.rehearsal, Rehearsal::Reports));
    let mint = asked(&Command::Keys(Keyed::Mint {
        name: "ha".to_owned(),
        scope: "read".to_owned(),
        purpose: "other".to_owned(),
        by: Minter::Operator,
        claim: false,
        proved_by: None,
    }));
    assert!(matches!(mint.rehearsal, Rehearsal::Cannot(_)));
    let collect = asked(&Command::Keys(Keyed::Collect {
        claim: "lfc_".to_owned(),
        from: None,
    }));
    assert!(matches!(collect.rehearsal, Rehearsal::Cannot(_)));
}

/// Asking the back-ends rehearses as each request is: a listing and a member's own read
/// read, and every change and a test each say what they would come to.
#[test]
fn asking_the_back_ends_rehearses_as_each_request_is() {
    use crate::model::BackendKind;
    use crate::notify::configuring::{Adding, Asked as Notifying, Setting};
    let id = || "home-ntfy".to_owned();
    for (command, named) in [
        (Notifying::List, "notify list"),
        (
            Notifying::Mine {
                member: "ana".to_owned(),
            },
            "notify mine",
        ),
    ] {
        let rehearsed = asked(&Command::Notify(command));
        assert_eq!(rehearsed.named, named);
        assert!(matches!(rehearsed.rehearsal, Rehearsal::Reads));
    }
    for (command, named) in [
        (
            Notifying::Add(Box::new(Adding {
                id: id(),
                kind: BackendKind::Webhook,
                url: "http://hooks.home/alerts".to_owned(),
                topic: None,
                token: None,
                reader: None,
                hosted: false,
                attempts: None,
                delays: None,
            })),
            "notify add",
        ),
        (
            Notifying::Set(Box::new(Setting {
                id: id(),
                ..Setting::default()
            })),
            "notify set",
        ),
        (Notifying::Test { id: id() }, "notify test"),
        (Notifying::Remove { id: id() }, "notify remove"),
        (
            Notifying::Choose {
                member: "ana".to_owned(),
                backends: vec![id()],
            },
            "notify choose",
        ),
    ] {
        let rehearsed = asked(&Command::Notify(command));
        assert_eq!(rehearsed.named, named);
        assert!(matches!(rehearsed.rehearsal, Rehearsal::Reports));
    }
}
