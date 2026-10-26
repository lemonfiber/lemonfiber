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
    /// entering it.
    #[serde(default, skip_serializing_if = "core::ops::Not::not")]
    pub minted: bool,
    /// The capability of the plugin's own that a credential crossing to it is needed to fill.
    #[serde(default, rename = "for", skip_serializing_if = "Option::is_none")]
    pub serves: Option<String>,
    /// Where the service a part moves in place of wrote this credential itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carried_from: Option<CarriedFrom>,
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
#[serde(rename_all = "lowercase")]
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
