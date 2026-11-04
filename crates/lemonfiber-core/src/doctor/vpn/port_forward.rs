//! The pure port-forward logic: what the egress guard said it was forwarded, and what a
//! missing port can honestly be called.

use super::findings::finding;
use super::VpnCheck;
use super::NOT_ENABLED;
use crate::doctor::{Finding, Verdict};
use crate::error::codes::vpn::NO_FORWARDED_PORT;
use crate::error::{Problem, Remedy, State};

/// What the egress guard answered asked for the port its provider forwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grant {
    /// A port was granted.
    Port(u16),
    /// The guard answered, naming no usable port, so none was granted.
    Absent,
    /// The guard could not be asked: none fills the stack's egress, it speaks no contract
    /// or it did not answer, so whether a port exists is unknown rather than absent.
    Unreadable,
}

impl Grant {
    /// What `forwarded` came to: the guard's answer where it gave one, a zero read as no
    /// port, and nothing where it could not be asked.
    #[must_use]
    pub fn of(forwarded: Option<Option<u16>>) -> Self {
        match forwarded {
            Some(Some(port)) if port != 0 => Self::Port(port),
            Some(_) => Self::Absent,
            None => Self::Unreadable,
        }
    }

    /// The port granted, where one was.
    #[must_use]
    pub const fn port(self) -> Option<u16> {
        match self {
            Self::Port(port) => Some(port),
            Self::Absent | Self::Unreadable => None,
        }
    }
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

impl VpnCheck {
    /// The port-forward finding: whether a port was granted, where the operator
    /// asked for one.
    ///
    /// Independent of leak detection — the port is what the egress guard answered, not an
    /// IP-echo comparison — so it is still established where the operator has switched
    /// leak detection off.
    ///
    /// A guard that could not be asked makes the read unknown rather than absent. An
    /// answer naming no port, or a zero, is a port that was not granted, which for an
    /// enabled provider is the failure this check exists for.
    pub(super) fn port_forward_finding(&self) -> Finding {
        let verdict = if self.port_forward.enabled {
            match self.granted {
                Grant::Port(port) => Verdict::Pass {
                    note: Some(format!("forwarded port {port}")),
                },
                Grant::Absent => no_port(self.port_forward.provider.as_deref()),
                Grant::Unreadable => Verdict::Unverified {
                    reason: "the egress guard did not answer which port it was forwarded, which \
                             its tunnel being down and the guard not running both produce"
                        .to_owned(),
                    remedy: Remedy::new("Confirm the tunnel is up, then run this again"),
                },
            }
        } else {
            Verdict::Skipped {
                reason: NOT_ENABLED.to_owned(),
            }
        };
        finding("vpn.port-forward", "forwarded port", verdict)
    }
}

#[cfg(test)]
mod tests;
