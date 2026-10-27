//! The values a service's environment holds, and what is wrong with one.

use serde::{Deserialize, Serialize};

use super::{is_api_name, MEDIA};
use crate::validate::is_core_name;

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
    /// What a recording gives the service in the operator's place, where only the
    /// operator holds the value and the service needs one to start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_with: Option<RecordedWith>,
    /// The variable an earlier release kept its value under in the stack's environment
    /// file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub was: Option<String>,
}

/// What a recording gives a service in the operator's place: a constant reaching no third
/// party, or, written `"minted"`, a value made for that run alone.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(from = "Constant", into = "Constant")]
#[schemars(with = "Constant")]
pub enum RecordedWith {
    /// [`RECORDED_MINTED`] random bytes made for that run alone, written in base64.
    Minted,
    /// A constant reaching no third party.
    Constant(Constant),
}

/// How a manifest writes [`RecordedWith::Minted`].
pub const MINTED_STAND_IN: &str = "minted";

/// How many random bytes a [`RecordedWith::Minted`] stand-in is made of.
pub const RECORDED_MINTED: usize = 32;

impl RecordedWith {
    /// What is wrong with the stand-in, where it holds a character no reader can see.
    #[must_use]
    pub fn unshown(&self) -> Option<String> {
        match self {
            Self::Constant(constant) => unshown(constant),
            Self::Minted => None,
        }
    }
}

impl From<Constant> for RecordedWith {
    fn from(constant: Constant) -> Self {
        match constant {
            Constant::Text(text) if text == MINTED_STAND_IN => Self::Minted,
            other => Self::Constant(other),
        }
    }
}

impl From<RecordedWith> for Constant {
    fn from(recorded: RecordedWith) -> Self {
        match recorded {
            RecordedWith::Minted => Self::Text(MINTED_STAND_IN.to_owned()),
            RecordedWith::Constant(constant) => constant,
        }
    }
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

/// A value a service holds, declared before anything captures or enters it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Secret {
    /// What the value is, within its namespace.
    pub id: String,
    /// Whose credential it is.
    pub of: String,
    /// What holding it is for.
    pub why: String,
    /// Whether lemonfiber makes the value at install, rather than anything capturing or
    /// entering it, and in what shape.
    #[serde(default, skip_serializing_if = "Minted::is_not")]
    #[schemars(with = "MintedAs")]
    pub minted: Minted,
    /// The capability of the plugin's own that a credential crossing to it is needed to fill.
    #[serde(default, rename = "for", skip_serializing_if = "Option::is_none")]
    pub serves: Option<String>,
    /// Where the service a part moves in place of wrote this credential itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carried_from: Option<CarriedFrom>,
    /// What a recording gives the service in the operator's place, where only the
    /// operator holds the value and the service needs one to start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_with: Option<RecordedWith>,
    /// The variable an earlier release kept its value under in the stack's environment
    /// file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub was: Option<String>,
}

/// Where a service wrote a credential itself: a file inside its container, the format it
/// is written in and the dotted path to the value.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "SecretCarriedFrom")]
pub struct CarriedFrom {
    /// The file, as a path inside the service's container beneath its configuration
    /// directory.
    pub file: String,
    /// The format the file is written in.
    pub format: Format,
    /// The dotted path to the value in the file; for `ini`, its section and entry.
    pub key: String,
}

/// A format a service writes its configuration in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "SecretFormat")]
pub enum Format {
    /// XML.
    Xml,
    /// JSON.
    Json,
    /// YAML.
    Yaml,
    /// INI.
    Ini,
}

/// Whether lemonfiber makes a secret's value, and in what shape.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(from = "MintedAs", into = "MintedAs")]
pub enum Minted {
    /// Something captures or enters it.
    #[default]
    Not,
    /// 32 lowercase hexadecimal characters.
    Key,
    /// 32 characters holding every character class a password rule asks for.
    Password,
}

impl Minted {
    /// Whether lemonfiber makes the value.
    #[must_use]
    pub const fn is(self) -> bool {
        !matches!(self, Self::Not)
    }

    /// Whether something captures or enters the value instead.
    #[must_use]
    pub const fn is_not(&self) -> bool {
        matches!(self, Self::Not)
    }
}

/// How a manifest writes [`Minted`]: `true` or `false`, or the shape it is made in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
#[schemars(rename = "SecretMinted")]
pub enum MintedAs {
    /// Made as a key, or not made.
    Flag(bool),
    /// Made in the shape named.
    Shaped(MintedShape),
}

/// A shape a minted value is made in, beyond a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "SecretMintedShape")]
pub enum MintedShape {
    /// A password: every character class a password rule asks for.
    Password,
}

impl From<MintedAs> for Minted {
    fn from(written: MintedAs) -> Self {
        match written {
            MintedAs::Flag(false) => Self::Not,
            MintedAs::Flag(true) => Self::Key,
            MintedAs::Shaped(MintedShape::Password) => Self::Password,
        }
    }
}

impl From<Minted> for MintedAs {
    fn from(minted: Minted) -> Self {
        match minted {
            Minted::Not => Self::Flag(false),
            Minted::Key => Self::Flag(true),
            Minted::Password => Self::Shaped(MintedShape::Password),
        }
    }
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

/// What is wrong with an ask for an address: a capability outside the core's, a medium
/// outside the vocabulary or an API that is not one lowercase word.
pub(super) fn narrowed(url: &UrlOf) -> Option<String> {
    let mut wrong = Vec::new();
    if !is_core_name(&url.url_of) {
        wrong.push(format!("{} is not a core capability", url.url_of));
    }
    if let Some(medium) = url
        .files
        .as_deref()
        .filter(|medium| !MEDIA.contains(medium))
    {
        wrong.push(format!(
            "files {medium}, which is not one of {}",
            MEDIA.join(", ")
        ));
    }
    if let Some(api) = url.native.as_deref().filter(|api| !is_api_name(api)) {
        wrong.push(format!("native {api} is not one lowercase word"));
    }
    (!wrong.is_empty()).then(|| format!("asks for an address where {}", wrong.join("; ")))
}

/// What is wrong with a constant that holds a character no reader can see.
pub(super) fn unshown(constant: &Constant) -> Option<String> {
    match constant {
        Constant::Text(text) if text.chars().any(char::is_control) => {
            Some(format!("{text:?} holds a control character"))
        }
        Constant::Text(_) | Constant::Number(_) | Constant::Flag(_) => None,
    }
}
