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
