use serde::Serialize;

/// Every setting and secret the stack's services read, and what changing a setting came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct SettingsReport {
    /// Every setting, in name order.
    pub settings: Vec<SettingRead>,
    /// Every secret a service's environment names, in name order, never with its value.
    pub secrets: Vec<SecretRead>,
    /// The change asked for, where one was.
    pub change: Option<SettingChange>,
    /// Whether this was a rehearsal.
    pub rehearsed: bool,
    /// The operator's overlay and every service it changes, where there is one: merged
    /// over the project this run would start, and held to nothing lemonfiber vets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unvetted: Option<crate::model::Unvetted>,
}

/// One setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct SettingRead {
    /// Its name.
    pub name: String,
    /// Whose it is.
    pub whose: Owner,
    /// Each service that reads it.
    pub readers: Vec<Reader>,
    /// The operator's value, where one is recorded.
    pub value: Option<String>,
    /// The environment file's key it is held under, where it is held there.
    pub environment: Option<String>,
}

/// One secret, without its value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct SecretRead {
    /// Its name.
    pub name: String,
    /// Whose it is.
    pub whose: Owner,
    /// Each service that reads it.
    pub readers: Vec<Reader>,
    /// Where its value came from, where one is held or granted.
    pub from: Option<SecretFrom>,
    /// Whether a value is held.
    pub held: bool,
}

/// Whose a setting or a secret is.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, schemars::JsonSchema)]
#[serde(tag = "of", rename_all = "kebab-case")]
pub enum Owner {
    /// The core's.
    Core,
    /// A plugin's, or a stack service's, by its namespace.
    Namespace {
        /// The namespace.
        namespace: String,
    },
}

/// Where a secret's value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SecretFrom {
    /// The operator entered it.
    Operator,
    /// A recipe captured it.
    Recipe,
    /// It is another service's credential, granted to cross.
    Granted,
    /// lemonfiber made it at install.
    Minted,
}

/// One service reading a setting or a secret.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, schemars::JsonSchema)]
pub struct Reader {
    /// The service.
    pub service: String,
    /// The variable it is read into.
    pub variable: String,
    /// What the service's manifest declares while no value is recorded.
    pub default: Option<String>,
}

/// A change to one or more settings, made in one act.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct SettingChange {
    /// Each setting the act moves, in name order.
    pub settings: Vec<SettingMove>,
    /// Every service whose entry the change regenerates.
    pub regenerates: Vec<String>,
    /// Every running service the change restarts.
    pub restarts: Vec<String>,
    /// Every service held back that the change starts, having given the last thing it
    /// waited for.
    pub starts: Vec<String>,
    /// Whether it was written.
    pub applied: bool,
    /// What this reading names itself, part by part.
    pub agreement: String,
    /// Why the services it restarts were not restarted, where they were not.
    pub unrestarted: Option<String>,
    /// The services held back once the change is made, each with what it waits for.
    pub waiting: Vec<Waiting>,
}

/// One setting a change moves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct SettingMove {
    /// The setting.
    pub setting: String,
    /// What it holds now.
    pub was: Option<String>,
    /// What it is to hold, or nothing where it is cleared.
    pub value: Option<String>,
    /// What moving it costs, where it costs anything.
    pub consequence: Option<String>,
}

/// A service held back until the operator gives what it needs to start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Waiting {
    /// The service.
    pub service: String,
    /// Everything it waits for.
    pub wants: Vec<Wanted>,
}

/// One thing a service waits for, never with its value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Wanted {
    /// A setting only the operator holds, or one holding a value outside its choices.
    Setting {
        /// The setting.
        setting: String,
        /// The values it accepts, where it declares choices.
        choices: Option<Vec<String>>,
    },
    /// A secret nobody has entered.
    Secret {
        /// The secret, by its name in the secret store.
        secret: String,
    },
    /// The service it runs through, which is held back itself.
    Service {
        /// That service.
        service: String,
    },
}

impl Wanted {
    /// What it is, as a sentence names it, never with a value.
    #[must_use]
    pub fn said(&self) -> String {
        match self {
            Self::Setting {
                setting,
                choices: Some(choices),
            } if choices.is_empty() => format!(
                "the setting {setting}, which has no choice until the settings it depends on \
                 change"
            ),
            Self::Setting {
                setting,
                choices: Some(choices),
            } => format!("the setting {setting}, one of {}", choices.join(", ")),
            Self::Setting {
                setting,
                choices: None,
            } => format!("the setting {setting}"),
            Self::Secret { secret } => format!("the secret {secret}"),
            Self::Service { service } => format!("{service}, which it runs through"),
        }
    }
}

impl Waiting {
    /// The service and everything it waits for, as one sentence names them.
    #[must_use]
    pub fn said(&self) -> String {
        let wants: Vec<String> = self.wants.iter().map(Wanted::said).collect();
        format!("{} waits for {}", self.service, wants.join("; "))
    }
}
