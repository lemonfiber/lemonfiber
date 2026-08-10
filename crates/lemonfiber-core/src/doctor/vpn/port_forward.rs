//! The pure port-forward logic: reading the gateway's status file and deciding
//! what a missing port can honestly be called.

use super::findings::finding;
use super::NOT_ENABLED;
use crate::config::PortForward;
use crate::doctor::{Finding, Verdict};
use crate::error::{Code, Problem, Remedy, Severity, State};

/// Raised when port forwarding was asked for but the provider granted no port.
pub const NO_FORWARDED_PORT: Code = Code::new("VPN-4");

/// What the gateway's forwarded-port status file amounted to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Grant {
    /// A port was granted.
    Port(u16),
    /// The file was read but names no usable port, so none was granted.
    Absent,
    /// The file could not be read: the container is down or the engine is
    /// unreachable, so whether a port exists is unknown rather than absent.
    Unreadable,
}

/// How much lemonfiber knows about a provider's port forwarding, which decides
/// what a missing port can honestly be called.
enum Knowledge {
    /// `ProtonVPN`, whose trap — port forwarding and a P2P server both chosen when
    /// the `WireGuard` configuration is generated — is specific and nameable.
    Proton,
    /// A provider known to offer port forwarding, but without a lemonfiber-specific
    /// trap to name beyond the generic one.
    Forwarding,
    /// A provider lemonfiber has no port-forwarding knowledge of, so a missing
    /// port cannot be explained without guessing.
    Unknown,
}

/// Read the status file's contents as a granted port.
///
/// A file that is missing makes `cat` exit non-zero with nothing on stdout, and
/// the release path writes a literal `0`; both are read as no port granted rather
/// than as a failure to read, because the file was reachable — it simply names no
/// port. Only an engine or container fault, handled before this, is `Unreadable`.
pub(super) fn parse_grant(stdout: &str) -> Grant {
    match stdout.trim().parse::<u16>() {
        Ok(port) if port != 0 => Grant::Port(port),
        _ => Grant::Absent,
    }
}

/// The verdict for an enabled provider that granted no port.
///
/// A provider lemonfiber knows forwards ports gets a failure — degraded, not
/// broken: nothing is leaking, but peers cannot reach the client. `ProtonVPN`'s
/// specific trap is named first where it is the provider. A provider lemonfiber
/// has no knowledge of is left `unverified` rather than blamed, because the cause
/// cannot be named without guessing.
pub(super) fn no_port(provider: Option<&str>) -> Verdict {
    match knowledge(provider) {
        Knowledge::Proton => Verdict::Warn(proton_trap()),
        Knowledge::Forwarding => Verdict::Warn(generic_trap()),
        Knowledge::Unknown => Verdict::Unverified {
            reason: "port forwarding is enabled but no port was granted, and this is not a \
                     provider lemonfiber has specific guidance for, so the cause cannot be named \
                     without guessing"
                .to_owned(),
            remedy: Remedy::new(
                "Confirm your provider supports port forwarding and that it was enabled when the \
                 VPN credentials were generated",
            ),
        },
    }
}

/// How much lemonfiber knows about a provider, by the name Gluetun uses for it.
fn knowledge(provider: Option<&str>) -> Knowledge {
    match provider {
        Some("protonvpn" | "proton") => Knowledge::Proton,
        Some(
            "private internet access"
            | "pia"
            | "privateinternetaccess"
            | "privatevpn"
            | "perfect privacy"
            | "perfectprivacy",
        ) => Knowledge::Forwarding,
        _ => Knowledge::Unknown,
    }
}

/// `ProtonVPN`'s trap, named first: the tunnel connects but the port never arrives
/// because forwarding was not chosen at configuration time.
fn proton_trap() -> Problem {
    Problem::new(
        NO_FORWARDED_PORT,
        Severity::Warning,
        "The VPN granted no forwarded port",
        "The tunnel is up, but no port was forwarded, so peers cannot open connections to your \
         client and both download connectivity and seeding are reduced. With ProtonVPN the usual \
         cause is that port forwarding (NAT-PMP) was not enabled, or a non-P2P server was chosen, \
         when the WireGuard configuration was generated — the tunnel still connects, only the port \
         never arrives. It cannot be fixed at runtime.",
        Remedy::new(
            "Regenerate the ProtonVPN WireGuard credentials with NAT-PMP enabled and a P2P server, \
             then replace WIREGUARD_PRIVATE_KEY",
        )
        .with_detail("account.protonvpn.com → Downloads → WireGuard: enable NAT-PMP, pick a P2P server"),
    )
    .in_state(State::Guided)
}

/// The trap for a provider that forwards ports but has no lemonfiber-specific
/// note: state the consequence and where the setting usually lives, without
/// inventing a cause.
fn generic_trap() -> Problem {
    Problem::new(
        NO_FORWARDED_PORT,
        Severity::Warning,
        "The VPN granted no forwarded port",
        "The tunnel is up, but no port was forwarded, so peers cannot open connections to your \
         client and both download connectivity and seeding are reduced. On providers that support \
         port forwarding it usually has to be enabled at the point the credentials are generated, \
         not afterwards.",
        Remedy::new(
            "Confirm port forwarding is enabled for this provider, and regenerate the VPN \
             credentials with it enabled if it was not",
        ),
    )
    .in_state(State::Guided)
}

/// The port-forward finding when the gateway cannot be read at all, because the
/// engine is unreachable: unverified where a port was expected, and still just
/// not-applicable where forwarding was never enabled.
pub(super) fn port_forward_offline(port_forward: &PortForward) -> Finding {
    let verdict = if port_forward.enabled {
        Verdict::Unverified {
            reason: "the container engine could not be reached, so the forwarded port \
                     could not be read"
                .to_owned(),
            remedy: Remedy::new("Start the container engine, then run this again"),
        }
    } else {
        Verdict::Skipped {
            reason: NOT_ENABLED.to_owned(),
        }
    };
    finding("vpn.port-forward", "forwarded port", verdict)
}

#[cfg(test)]
mod tests {
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
}
