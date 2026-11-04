//! The privileged shapes lemonfiber writes for a service, from a closed set.

use serde::{Deserialize, Serialize};

use super::{EGRESS_GUARD, MEDIA_SERVE, RENDER_DEVICE};
use crate::validate::{GATEWAY_GRANT, TUNNEL_DEVICE};

/// A privileged shape lemonfiber writes for a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "PluginShape")]
pub enum Shape {
    /// `NET_ADMIN` and the tunnel device.
    EgressGuard,
    /// The host's render devices and the group that owns them.
    Render,
}

impl Shape {
    /// Every shape lemonfiber writes.
    pub const ALL: [Self; 2] = [Self::EgressGuard, Self::Render];

    /// The name a manifest and an approval give the shape.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::EgressGuard => "egress-guard",
            Self::Render => "render",
        }
    }

    /// The kernel capabilities the shape adds.
    #[must_use]
    pub const fn grants(self) -> &'static [&'static str] {
        match self {
            Self::EgressGuard => &[GATEWAY_GRANT],
            Self::Render => &[],
        }
    }

    /// The devices the shape adds.
    #[must_use]
    pub const fn devices(self) -> &'static [&'static str] {
        match self {
            Self::EgressGuard => &[TUNNEL_DEVICE],
            Self::Render => &[RENDER_DEVICE],
        }
    }

    /// Whether the shape adds the group owning its devices on the engine's host.
    #[must_use]
    pub const fn grouped(self) -> bool {
        matches!(self, Self::Render)
    }

    /// The capability a service declaring the shape provides, or whose adapter fronts it.
    #[must_use]
    pub const fn serves(self) -> &'static str {
        match self {
            Self::EgressGuard => EGRESS_GUARD,
            Self::Render => MEDIA_SERVE,
        }
    }

    /// What approving this shape for `service` is written as.
    #[must_use]
    pub fn approval(self, service: &str) -> String {
        format!("{}@{service}", self.name())
    }
}
