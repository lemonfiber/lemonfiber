//! The container fields a service declares, one closed and typed set for a stack's
//! service and a plugin's alike, and the rules every one of them is held to.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::validate::{is_core_name, GATEWAY_GRANT, TUNNEL_DEVICE};

/// Where a service's configuration directory lands when it names nowhere.
pub const CONFIGURATION: &str = "/config";

/// Where the single data mount lands inside every container that takes data.
pub const DATA: &str = "/data";

/// The host's render devices, which the `render` shape adds where the host has them.
pub const RENDER_DEVICE: &str = "/dev/dri";

/// The variables lemonfiber writes itself, which no manifest may name.
pub const RESERVED: [&str; 3] = ["TZ", "PUID", "PGID"];

/// The settings the core publishes to every service.
pub const PUBLISHED: [&str; 2] = ["domain", "lan-host"];

/// The egress guard's capability.
pub const EGRESS_GUARD: &str = "network.egress-guard";

/// The media server's capability.
pub const MEDIA_SERVE: &str = "media.serve";

/// The capabilities a service may run inside the network of.
pub const VIA: [&str; 1] = [EGRESS_GUARD];

/// What separates a setting's namespace from its name.
const NAMESPACED: char = ':';

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

/// A value written exactly as declared.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
#[schemars(rename = "ServiceConstant")]
pub enum Constant {
    /// A string.
    Text(String),
    /// An integer.
    Number(i64),
    /// A boolean.
    Flag(bool),
}

/// An operator setting, with what is written while none is recorded.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ServiceSetting")]
pub struct Setting {
    /// The setting's name.
    pub setting: String,
    /// What is written while the operator has recorded no value.
    pub default: Constant,
}

/// The address of a capability's filler.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ServiceUrlOf")]
pub struct UrlOf {
    /// The capability asked for.
    pub url_of: String,
    /// The medium the filler asked for files.
    #[serde(default)]
    pub files: Option<String>,
    /// The upstream's own API, where the ask reaches the upstream an adapter fronts.
    #[serde(default)]
    pub native: Option<String>,
}

/// A secret the manifest declares.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ServiceSecretOf")]
pub struct SecretOf {
    /// The declared secret's name.
    pub secret: String,
}

/// One environment variable's value.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
#[schemars(rename = "ServiceValue")]
pub enum Value {
    /// A constant.
    Constant(Constant),
    /// An operator setting.
    Setting(Setting),
    /// A capability's filler's address.
    Url(UrlOf),
    /// A declared secret.
    Secret(SecretOf),
}

/// Who the container runs as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "ServiceUserMode")]
pub enum UserMode {
    /// No user: the image decides.
    Image,
    /// `PUID` and `PGID` set to the operator's.
    PuidEnv,
    /// The operator's uid and gid as the container's user.
    Uid,
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

/// The network a service runs inside instead of its own.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ServiceNetwork")]
pub struct Network {
    /// The capability whose filler's network it runs inside.
    pub via: String,
}

/// The container fields an install record keeps for a service, beyond those every
/// record already holds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ServiceFields")]
pub struct Fields {
    /// Its further published ports.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub publish: Vec<Publish>,
    /// Whether its configuration directory is read-only.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub config_ro: bool,
    /// Its further mounts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mount: Vec<Mount>,
    /// Its scratch paths in memory.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tmpfs: Vec<String>,
    /// Its environment.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, Value>,
    /// Who it runs as, where declared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_mode: Option<UserMode>,
    /// How tightly it is held.
    #[serde(default, skip_serializing_if = "Posture::is_standard")]
    pub posture: Posture,
    /// Its memory ceiling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mem_limit_mib: Option<u32>,
    /// The network it runs inside.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<Network>,
}

impl Fields {
    /// Whether it declares nothing beyond what every record holds.
    #[must_use]
    pub fn is_plain(&self) -> bool {
        *self == Self::default()
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
    /// Its scratch paths in memory.
    pub tmpfs: &'a [String],
    /// Its environment.
    pub env: &'a BTreeMap<String, Value>,
    /// The secrets its manifest declares.
    pub secrets: &'a [&'a str],
    /// Who it runs as, where declared.
    pub user_mode: Option<UserMode>,
    /// How tightly it is held.
    pub posture: Posture,
    /// Its memory ceiling.
    pub mem_limit_mib: Option<u32>,
    /// The network it runs inside.
    pub network: Option<&'a Network>,
    /// Its privileged shape.
    pub shape: Option<Shape>,
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
        match self.user_mode {
            Some(mode) => mode,
            None if self.speaks || self.posture == Posture::Confined => UserMode::Uid,
            None => UserMode::Image,
        }
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
        for port in self
            .port
            .into_iter()
            .chain(self.publish.iter().map(|it| it.port))
        {
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
            if !is_relative(&mount.from) {
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
                .find(|other| inside(&mount.from, &other.from))
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
                unshown(&setting.default),
                (!self.owns(&setting.setting)).then(|| {
                    format!(
                        "names setting {}, which is neither one the core publishes nor in the {} \
                         namespace",
                        setting.setting, self.namespace
                    )
                }),
            ),
            Value::Url(url) => (
                None,
                (!is_core_name(&url.url_of)).then(|| {
                    format!(
                        "asks for the address of {}, which is not a core capability",
                        url.url_of
                    )
                }),
            ),
            Value::Secret(secret) => (
                None,
                (!self.secrets.contains(&secret.secret.as_str())).then(|| {
                    format!(
                        "names secret {}, which the manifest does not declare",
                        secret.secret
                    )
                }),
            ),
        };
        shown.into_iter().chain(named)
    }

    fn owns(&self, setting: &str) -> bool {
        PUBLISHED.contains(&setting)
            || setting
                .split_once(NAMESPACED)
                .is_some_and(|(space, name)| space == self.namespace && !name.is_empty())
    }

    fn held(&self, found: &mut Vec<(String, String)>) {
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
        if !VIA.contains(&network.via.as_str()) {
            found.push((
                self.at("network.via"),
                format!(
                    "names {}, which no service may run inside the network of",
                    network.via
                ),
            ));
        }
        if self.provides.contains(&network.via) {
            found.push((
                self.at("network.via"),
                format!("names {}, which the service provides itself", network.via),
            ));
        }
    }
}

/// What is wrong with a constant that holds a character no reader can see.
fn unshown(constant: &Constant) -> Option<String> {
    match constant {
        Constant::Text(text) if text.chars().any(char::is_control) => {
            Some(format!("{text:?} holds a control character"))
        }
        Constant::Text(_) | Constant::Number(_) | Constant::Flag(_) => None,
    }
}

/// What is wrong with `path` as a target inside a container, if anything.
fn target(path: &str, takes_data: bool) -> Option<&'static str> {
    if !path.starts_with('/') || path == "/" || path.contains("..") || !is_plain(path) {
        return Some(
            "is not one plain absolute path: not the root, with no \"..\", in letters, digits \
             and \"._/-\" alone",
        );
    }
    let in_data = path == DATA || path.starts_with(&format!("{DATA}/"));
    (takes_data && in_data).then_some("is inside the data mount")
}

/// Whether the source `path` lies inside the source `other`, and is not it.
fn inside(path: &str, other: &str) -> bool {
    path.len() > other.len()
        && path.starts_with(other)
        && path.as_bytes().get(other.len()) == Some(&b'/')
}

fn is_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && is_plain(path)
        && path
            .split('/')
            .all(|step| !step.is_empty() && step != "." && step != "..")
}

/// Whether `text` is written only in letters, digits, `.`, `_`, `/` and `-`.
#[must_use]
pub fn is_plain(text: &str) -> bool {
    text.chars()
        .all(|letter| letter.is_ascii_alphanumeric() || matches!(letter, '.' | '_' | '/' | '-'))
}

fn is_variable(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|first| first.is_ascii_uppercase() || first == '_')
        && name
            .chars()
            .all(|letter| letter.is_ascii_uppercase() || letter.is_ascii_digit() || letter == '_')
}

mod fields;
#[cfg(test)]
mod tests;
