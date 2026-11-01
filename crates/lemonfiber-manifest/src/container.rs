//! The container fields a service declares, one closed and typed set for a stack's
//! service and a plugin's alike, and the rules every one of them is held to.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::validate::{GATEWAY_GRANT, TUNNEL_DEVICE};

mod certified;
pub mod choices;
pub mod earlier;
mod paths;
mod ports;
mod record;
mod user;
mod values;
mod words;

pub use certified::{Certificate, MOST_NAMES};
use paths::target;
pub use paths::{is_plain, is_source, is_target, lies_inside};
pub use ports::{doubled, exposed, exposes, inside, published, reached, Reach};
pub use record::Fields;
pub use user::{IdVariables, UserMode, UserModeName, UserModeWritten};
use values::{narrowed, unshown};
pub use values::{
    CarriedFrom, Constant, Format, Minted, MintedAs, RecordedWith, Secret, SecretOf, Setting,
    UrlOf, Value, MINTED_STAND_IN, RECORDED_MINTED,
};
pub use words::is_api_name;
use words::is_variable;

/// Where a service's configuration directory lands when it names nowhere.
pub const CONFIGURATION: &str = "/config";

/// Where the single data mount lands inside every container that takes data.
pub const DATA: &str = "/data";

/// The host's render devices, which the `render` shape adds where the host has them.
pub const RENDER_DEVICE: &str = "/dev/dri";

/// The variable lemonfiber writes the operator's time zone into.
pub const ZONE: &str = "TZ";

/// The variable an image following the LinuxServer.io convention reads the operator's uid
/// from.
pub const PUID: &str = "PUID";

/// The variable an image following the LinuxServer.io convention reads the operator's gid
/// from.
pub const PGID: &str = "PGID";

/// The variables lemonfiber writes itself, which no manifest may name.
pub const RESERVED: [&str; 3] = [ZONE, PUID, PGID];

/// The core-published setting holding the operator's domain.
pub const DOMAIN: &str = "domain";

/// The core-published setting holding the address the household reaches this machine at.
pub const LAN_HOST: &str = "lan-host";

/// The settings the core publishes to every service.
pub const PUBLISHED: [&str; 2] = [DOMAIN, LAN_HOST];

/// The egress guard's capability.
pub const EGRESS_GUARD: &str = "network.egress-guard";

/// The media server's capability.
pub const MEDIA_SERVE: &str = "media.serve";

/// The stream guard's capability, whose filler the core hands a certificate.
pub const STREAM_GUARD: &str = "stream.guard";

/// The id lemonfiber runs its pass service under, which no service of the stack or of a
/// plugin may take.
pub const PASS_SERVICE: &str = "pass";

/// The media a service files, in the stack manifest's vocabulary.
pub const MEDIA: [&str; 7] = [
    "tv",
    "movies",
    "music",
    "books",
    "comics",
    "audiobooks",
    "podcasts",
];

/// The capabilities a service may run inside the network of.
pub const VIA: [&str; 1] = [EGRESS_GUARD];

/// What separates a setting's or a secret's namespace from its name.
pub const NAMESPACED: char = ':';

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

/// A further port a service publishes on its tier.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ServicePublish")]
pub struct Publish {
    /// Where it is published on the host.
    pub port: u16,
    /// Where it answers inside the container, `port` where absent.
    #[serde(default)]
    pub listens: Option<u16>,
}

/// A further path mounted from the service's own mount tree.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ServiceMount")]
pub struct Mount {
    /// The relative path inside the mount tree.
    pub from: String,
    /// The absolute path inside the container.
    pub to: String,
    /// Whether it is mounted read-only.
    #[serde(default)]
    pub ro: bool,
}

/// How tightly the container is held.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "ServicePosture")]
pub enum Posture {
    /// Six kernel capabilities, no privilege gained, a bounded process count.
    #[default]
    Standard,
    /// A read-only root and no kernel capability at all, besides.
    Confined,
}

/// The network a service runs inside instead of its own: its filler's, or a fence's.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ServiceNetwork")]
pub struct Network {
    /// The capability whose filler's network it runs inside.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<String>,
    /// Whether it fetches whatever address it is handed, and so runs inside a fence that
    /// refuses every private address.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fenced: bool,
}

impl Network {
    /// The capability whose filler's network it runs inside, where it takes one.
    #[must_use]
    pub fn via(&self) -> Option<&str> {
        self.via.as_deref()
    }
}

impl Posture {
    /// Whether it is the standard posture.
    #[must_use]
    pub const fn is_standard(&self) -> bool {
        matches!(self, Self::Standard)
    }
}

/// One service's container fields, borrowed from whichever manifest declares them.
#[derive(Debug, Clone, Copy)]
pub struct Container<'a> {
    /// The service's id.
    pub id: &'a str,
    /// The namespace its own settings are written in.
    pub namespace: &'a str,
    /// Where it is published on the host.
    pub port: Option<u16>,
    /// Where it answers inside the container.
    pub listens: Option<u16>,
    /// Its further published ports.
    pub publish: &'a [Publish],
    /// Whether it takes the data mount.
    pub takes_data: bool,
    /// Where its configuration directory lands.
    pub config_path: Option<&'a str>,
    /// Whether its configuration directory is read-only.
    pub config_ro: bool,
    /// Its further mounts.
    pub mounts: &'a [Mount],
    /// Where the configuration directory of the service it fronts is mounted.
    pub shares_config: Option<&'a str>,
    /// The paths beneath its configuration directory it rebuilds.
    pub rebuilt: &'a [String],
    /// The service of its own plugin it stands in front of.
    pub fronts: Option<&'a str>,
    /// Its scratch paths in memory.
    pub tmpfs: &'a [String],
    /// Its environment.
    pub env: &'a BTreeMap<String, Value>,
    /// The secrets its manifest declares.
    pub secrets: &'a [Secret],
    /// Who it runs as, where declared.
    pub user_mode: Option<&'a UserMode>,
    /// How tightly it is held.
    pub posture: Posture,
    /// Its memory ceiling.
    pub mem_limit_mib: Option<u32>,
    /// The network it runs inside.
    pub network: Option<&'a Network>,
    /// Its privileged shape.
    pub shape: Option<Shape>,
    /// The certificate lemonfiber makes for it, where it declares one.
    pub certificate: Option<&'a Certificate>,
    /// The capabilities it provides.
    pub provides: &'a [String],
    /// Whether it speaks a capability contract.
    pub speaks: bool,
}

impl Container<'_> {
    /// Where its configuration directory lands inside the container.
    #[must_use]
    pub fn configuration(&self) -> &str {
        self.config_path.unwrap_or(CONFIGURATION)
    }

    /// Who it runs as, the declared mode or the one its kind requires.
    #[must_use]
    pub fn user(&self) -> UserMode {
        UserMode::of(self.user_mode, self.speaks, self.posture)
    }

    /// Every rule its fields break, each as where and what.
    #[must_use]
    pub fn broken(&self) -> Vec<(String, String)> {
        let mut found = Vec::new();
        self.published(&mut found);
        self.placed(&mut found);
        self.mounted(&mut found);
        self.environed(&mut found);
        self.held(&mut found);
        self.routed(&mut found);
        found
    }

    fn at(&self, field: &str) -> String {
        format!("service {}.{field}", self.id)
    }

    fn published(&self, found: &mut Vec<(String, String)>) {
        if self.port.is_none() && !self.publish.is_empty() {
            found.push((
                self.at("publish"),
                "publishes further ports but declares no port".to_owned(),
            ));
        }
        let mut seen = Vec::new();
        for port in published(self.port, self.publish) {
            if seen.contains(&port) {
                found.push((self.at("publish"), format!("publishes {port} twice")));
            }
            seen.push(port);
        }
    }

    fn placed(&self, found: &mut Vec<(String, String)>) {
        if let Some(path) = self.config_path {
            if let Some(wrong) = target(path, true) {
                found.push((self.at("config_path"), format!("{path} {wrong}")));
            }
        }
    }

    fn mounted(&self, found: &mut Vec<(String, String)>) {
        let mut targets = vec![self.configuration()];
        for mount in self.mounts {
            let at = self.at(&format!("mount {}", mount.to));
            if !is_source(&mount.from) {
                found.push((
                    at.clone(),
                    format!(
                        "{} is not a relative path inside its own mount tree",
                        mount.from
                    ),
                ));
            }
            self.landed(at.clone(), &mount.to, &mut targets, found);
            if let Some(outer) = self
                .mounts
                .iter()
                .find(|other| lies_inside(&mount.from, &other.from))
            {
                found.push((
                    at,
                    format!(
                        "{} lies inside {}, another of its sources",
                        mount.from, outer.from
                    ),
                ));
            }
        }
        for path in self.tmpfs {
            self.landed(self.at(&format!("tmpfs {path}")), path, &mut targets, found);
        }
        if let Some(path) = self.shares_config {
            let at = self.at("shares_config");
            if self.fronts.is_none() {
                found.push((
                    at.clone(),
                    "shares the configuration of the service it fronts, and fronts none".to_owned(),
                ));
            }
            self.landed(at, path, &mut targets, found);
        }
        if let Some(certificate) = self.certificate {
            found.extend(
                certificate
                    .broken(self.provides)
                    .into_iter()
                    .map(|wrong| (self.at("certificate"), wrong)),
            );
        }
        for path in self.rebuilt {
            if !is_source(path) {
                found.push((
                    self.at(&format!("rebuilt {path}")),
                    format!("{path} is not a relative path beneath its configuration directory"),
                ));
            }
        }
    }

    fn landed<'a>(
        &self,
        at: String,
        to: &'a str,
        targets: &mut Vec<&'a str>,
        found: &mut Vec<(String, String)>,
    ) {
        if let Some(wrong) = target(to, self.takes_data) {
            found.push((at.clone(), format!("{to} {wrong}")));
        }
        if targets.contains(&to) {
            found.push((at, format!("{to} is the target of another mount")));
        }
        targets.push(to);
    }

    fn environed(&self, found: &mut Vec<(String, String)>) {
        for (name, value) in self.env {
            let at = self.at(&format!("env.{name}"));
            if !is_variable(name) {
                found.push((
                    at.clone(),
                    "is not upper-case letters, digits and \"_\", starting with a letter or \"_\""
                        .to_owned(),
                ));
            }
            if RESERVED.contains(&name.as_str()) {
                found.push((at.clone(), format!("{name} is lemonfiber's to write")));
            }
            found.extend(self.valued(value).map(|wrong| (at.clone(), wrong)));
        }
    }

    fn valued(&self, value: &Value) -> impl Iterator<Item = String> {
        let (shown, named) = match value {
            Value::Constant(constant) => (unshown(constant), None),
            Value::Setting(setting) => (
                unshown(&setting.default).or_else(|| {
                    setting
                        .recorded_with
                        .as_ref()
                        .and_then(RecordedWith::unshown)
                }),
                self.settled(&setting.setting),
            ),
            Value::Url(url) => (None, narrowed(url)),
            Value::Secret(secret) => (
                None,
                (!self.declares(&secret.secret)).then(|| {
                    format!(
                        "names secret {}, which the manifest does not declare",
                        secret.secret
                    )
                }),
            ),
        };
        shown.into_iter().chain(named)
    }

    fn settled(&self, setting: &str) -> Option<String> {
        if !self.owns(setting) {
            return Some(format!(
                "names setting {setting}, which is neither one the core publishes nor in the {} \
                 namespace",
                self.namespace
            ));
        }
        setting
            .split_once(NAMESPACED)
            .filter(|(_, name)| self.declares(name))
            .map(|_| format!("names setting {setting}, which a secret it declares is also named"))
    }

    fn declares(&self, secret: &str) -> bool {
        self.secrets.iter().any(|declared| declared.id == secret)
    }

    fn owns(&self, setting: &str) -> bool {
        PUBLISHED.contains(&setting)
            || setting
                .split_once(NAMESPACED)
                .is_some_and(|(space, name)| space == self.namespace && !name.is_empty())
    }

    fn held(&self, found: &mut Vec<(String, String)>) {
        if let Some(variables) = self.user_mode.and_then(UserMode::ids_in) {
            self.named_ids(variables, found);
        }
        if (self.speaks || self.posture == Posture::Confined) && self.user() != UserMode::Uid {
            found.push((
                self.at("user_mode"),
                "must be uid for a service that speaks a contract or is confined".to_owned(),
            ));
        }
        if let (Posture::Confined, Some(shape)) = (self.posture, self.shape) {
            found.push((
                self.at("shape"),
                format!(
                    "is confined and takes {}, which a confined service may not",
                    shape.name()
                ),
            ));
        }
        if self.mem_limit_mib == Some(0) {
            found.push((self.at("mem_limit_mib"), "is zero".to_owned()));
        }
    }

    fn routed(&self, found: &mut Vec<(String, String)>) {
        let Some(network) = self.network else {
            return;
        };
        let Some(via) = network.via() else {
            if !network.fenced {
                found.push((
                    self.at("network"),
                    "says neither via nor fenced, and a network is one or the other".to_owned(),
                ));
            }
            return;
        };
        if network.fenced {
            found.push((
                self.at("network"),
                "says both via and fenced, and each is a network namespace of its own".to_owned(),
            ));
        }
        if !VIA.contains(&via) {
            found.push((
                self.at("network.via"),
                format!("names {via}, which no service may run inside the network of"),
            ));
        }
        if self.provides.iter().any(|provided| provided == via) {
            found.push((
                self.at("network.via"),
                format!("names {via}, which the service provides itself"),
            ));
        }
    }
}

mod fields;
#[cfg(test)]
mod tests;
