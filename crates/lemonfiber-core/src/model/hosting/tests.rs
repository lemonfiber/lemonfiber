use super::{HostedCommand, Hosting};

#[test]
fn the_states_are_spelled_the_way_the_operator_reads_them() {
    let named = |standing| {
        serde_json::to_value(HostedCommand {
            name: "watch".to_owned(),
            guarantees: "guards the data location".to_owned(),
            command: "lemonfiber watch".to_owned(),
            standing,
            definition: None,
            runs: None,
            output: None,
            missing: None,
        })
        .ok()
        .and_then(|value| {
            value
                .get("standing")
                .and_then(|at| at.as_str())
                .map(str::to_owned)
        })
    };
    assert_eq!(named(Hosting::NotHosted).as_deref(), Some("not-hosted"));
    assert_eq!(named(Hosting::Hosted).as_deref(), Some("hosted"));
    assert_eq!(
        named(Hosting::InstalledUnverified).as_deref(),
        Some("installed-unverified")
    );
    assert_eq!(named(Hosting::Stopped).as_deref(), Some("stopped"));
    assert_eq!(named(Hosting::Orphaned).as_deref(), Some("orphaned"));
    assert_eq!(named(Hosting::Unsupported).as_deref(), Some("unsupported"));
}

#[test]
fn four_of_the_six_are_installed_and_only_one_of_them_is_keeping_anything() {
    for standing in [
        Hosting::Hosted,
        Hosting::InstalledUnverified,
        Hosting::Stopped,
        Hosting::Orphaned,
    ] {
        assert!(standing.installed(), "{standing:?} has a definition");
    }
    for standing in [Hosting::NotHosted, Hosting::Unsupported] {
        assert!(!standing.installed(), "{standing:?} has none");
    }
    assert!(Hosting::Hosted.keeping());
    for standing in [
        Hosting::NotHosted,
        Hosting::InstalledUnverified,
        Hosting::Stopped,
        Hosting::Orphaned,
        Hosting::Unsupported,
    ] {
        assert!(!standing.keeping(), "{standing:?} keeps nothing");
    }
}
