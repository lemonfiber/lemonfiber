use super::{knowledge, no_port, parse_grant, Grant, Knowledge};
use crate::doctor::Verdict;

#[test]
fn a_provider_that_simply_does_not_forward_is_never_reported_as_broken() {
    // The failure mode that matters most here: telling an operator their VPN
    // is faulty when it is working exactly as the provider sells it. They go
    // looking for a fault that does not exist, and learn the check guesses.
    assert!(matches!(knowledge(None), Knowledge::Unknown));
    assert!(
        matches!(no_port(None), Verdict::Unverified { .. }),
        "unknown means unverified, never a failure"
    );
}

#[test]
fn a_provider_whose_capability_is_unknown_assumes_nothing_either_way() {
    // Not present, not absent. Assuming presence blames a provider that never
    // offered it; assuming absence hides a real misconfiguration.
    let verdict = no_port(Some("some-vpn-nobody-here-has-heard-of"));
    assert!(matches!(verdict, Verdict::Unverified { .. }));
}

#[test]
fn a_provider_known_to_forward_is_told_what_usually_went_wrong() {
    // Degraded rather than broken: nothing is leaking, but peers cannot reach
    // the client, and that is a real cost worth naming.
    for provider in ["pia", "privatevpn", "perfect privacy"] {
        assert!(
            matches!(no_port(Some(provider)), Verdict::Warn(_)),
            "{provider}"
        );
    }
}

#[test]
fn protons_own_trap_is_named_rather_than_the_generic_advice() {
    // Port forwarding and a P2P server are both chosen when the WireGuard
    // configuration is generated, and getting either wrong looks identical
    // afterwards — which is why it is worth naming specifically.
    // Compared whole rather than unwrapped: a match with a fallback arm would
    // leave a branch no passing test can reach, and the verdicts differing is
    // the entire claim.
    let named = no_port(Some("proton"));
    let generic = no_port(Some("pia"));
    assert_ne!(named, generic, "the trap is specific, not the generic line");
    assert!(matches!(named, Verdict::Warn(_)));
}

#[test]
fn a_provider_is_matched_however_it_is_spelled() {
    // Capability, not an enumerated list of exact strings: an operator writing
    // the name their own way must not silently become an unknown provider.
    for spelling in ["protonvpn", "proton"] {
        assert!(
            matches!(knowledge(Some(spelling)), Knowledge::Proton),
            "{spelling}"
        );
    }
    for spelling in ["pia", "private internet access", "privateinternetaccess"] {
        assert!(
            matches!(knowledge(Some(spelling)), Knowledge::Forwarding),
            "{spelling}"
        );
    }
}

#[test]
fn the_status_file_reads_a_released_port_as_no_port_rather_than_a_failure() {
    // The release path writes a literal zero, and a missing file makes `cat`
    // exit non-zero with nothing. Both mean no port, not an unreadable one.
    assert_eq!(parse_grant("51413\n"), Grant::Port(51413));
    assert_eq!(parse_grant("0"), Grant::Absent);
    assert_eq!(parse_grant(""), Grant::Absent);
    assert_eq!(parse_grant("not a port"), Grant::Absent);
}
