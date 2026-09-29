use super::bundled;

/// A bundled service providing `media.serve`, with `claims` written under it.
fn serving(claims: &str) -> lemonfiber_manifest::Service {
    let written = format!(
        r#"
id = "jellyfin"
name = "Jellyfin"
profile = "library"
image = "jellyfin/jellyfin"
tag = "10.10.7"
criticality = "core"
license = "GPL-2.0-only"
upstream = "https://jellyfin.org"
last_release = "2026-01-01"
describes = "Serves the library"
without_it = "Nothing to watch on"
provides = ["media.serve"]
{claims}
"#
    );
    toml::from_str(&written).unwrap_or_else(|why| unreachable!("{why}"))
}

/// Both probes `media.serve` declares, bound under Jellyfin's own recordings.
const BOTH: &str = r#"
[[claim]]
capability = "media.serve"

[[claim.probe]]
id = "guarded"
request = { method = "GET", path = "/Items" }
expect = { status = 401 }
fixture = "recordings/jellyfin/guarded.json"

[[claim.probe]]
id = "catalogue"
request = { method = "GET", path = "/Items" }
expect = { status = 200, json_has_keys = ["Items"] }
fixture = "recordings/jellyfin/catalogue.json"
"#;

/// A claim bound in full, under the service's own recordings, is read and refused for
/// nothing.
#[test]
fn a_claim_bound_in_full_is_read_and_refused_for_nothing() {
    let read = bundled(&serving(BOTH));

    assert_eq!(read.violations, Vec::new());
    assert_eq!(read.claims.len(), 1);
    assert!(read.unclaimed.is_empty());
}

/// A capability with no claim is returned as unclaimed, not refused.
#[test]
fn a_capability_nothing_claims_is_unclaimed() {
    let read = bundled(&serving(""));

    assert_eq!(read.unclaimed, vec!["media.serve".to_owned()]);
    assert!(read.violations.is_empty());
}

/// The binding rules are the plugin's: a probe the capability declares and nothing
/// binds is refused by name.
#[test]
fn a_probe_left_unbound_is_refused_as_a_plugins_would_be() {
    let one = BOTH
        .split("[[claim.probe]]")
        .take(2)
        .collect::<Vec<_>>()
        .join("[[claim.probe]]");
    let read = bundled(&serving(&one));

    assert!(
        read.violations
            .iter()
            .any(|refused| refused.message.contains("binds no probe catalogue")),
        "{:?}",
        read.violations
    );
}

/// A claim for something the service does not provide is refused.
#[test]
fn a_claim_for_what_the_service_does_not_provide_is_refused() {
    let read = bundled(&serving(&BOTH.replace("media.serve", "download.usenet")));

    assert!(
        read.violations
            .iter()
            .any(|refused| refused.message.contains("not in this service's `provides`")),
        "{:?}",
        read.violations
    );
    assert_eq!(read.unclaimed, vec!["media.serve".to_owned()]);
}

/// A capability the vocabulary does not carry is refused by name.
#[test]
fn a_claim_for_a_name_the_vocabulary_lacks_is_refused() {
    let written = BOTH.replace("media.serve", "media.nothing");
    let service = serving(&written);
    let read = bundled(&lemonfiber_manifest::Service {
        provides: vec!["media.nothing".to_owned()],
        ..service
    });

    assert!(
        read.violations
            .iter()
            .any(|refused| refused.message.contains("names no capability")),
        "{:?}",
        read.violations
    );
}

/// Claiming one capability twice is refused.
#[test]
fn a_capability_claimed_twice_is_refused() {
    let read = bundled(&serving(&format!("{BOTH}{BOTH}")));

    assert!(
        read.violations
            .iter()
            .any(|refused| refused.message.contains("claimed twice")),
        "{:?}",
        read.violations
    );
}

/// A recording kept anywhere but the service's own directory is refused.
#[test]
fn a_recording_outside_the_services_own_directory_is_refused() {
    for elsewhere in [
        "recordings/sonarr/guarded.json",
        "guarded.json",
        "recordings/jellyfin/../x.json",
    ] {
        let read = bundled(&serving(
            &BOTH.replace("recordings/jellyfin/guarded.json", elsewhere),
        ));

        assert!(
            read.violations.iter().any(|refused| refused
                .message
                .contains("is not under recordings/jellyfin/")),
            "{elsewhere}: {:?}",
            read.violations
        );
    }
}

/// A table that is not a claim is refused, naming where it is.
#[test]
fn a_table_that_is_not_a_claim_is_refused_where_it_is() {
    let read = bundled(&serving("[[claim]]\nwhat = \"nothing\"\n"));

    assert!(
        read.violations
            .iter()
            .any(|refused| refused.location == "service jellyfin.claim[0]"),
        "{:?}",
        read.violations
    );
    assert!(read.claims.is_empty());
}
