use clap::Subcommand;

/// Changing the settings a service reads.
#[derive(Debug, Subcommand)]
pub enum SettingsCommand {
    /// Record a value for one or more settings at once, regenerating every entry that
    /// reads them.
    ///
    /// The settings named are checked together, so settings whose choices depend on each
    /// other move in one act. Named on its own it says what the change would come to — the
    /// entries it regenerates and the running services it restarts — changes nothing, and
    /// prints a name for that offer; answering with that name is the yes.
    Set {
        /// Each setting and its value as `SETTING=VALUE`, such as
        /// `gluetun:countries=Belgium`; the value is written exactly as given.
        #[arg(required = true, value_name = "SETTING=VALUE", value_parser = assigned)]
        values: Vec<(String, String)>,
        /// The offer being answered, as the run that made it printed it.
        #[arg(long, value_name = "NAME")]
        offer: Option<String>,
    },
    /// Take your value away from one or more settings, so each declared default is written
    /// again.
    ///
    /// Named on its own it says what the change would come to and prints a name for that
    /// offer, as `set` does.
    Clear {
        /// Each setting, such as `gluetun:countries`.
        #[arg(required = true, value_name = "SETTING")]
        settings: Vec<String>,
        /// The offer being answered, as the run that made it printed it.
        #[arg(long, value_name = "NAME")]
        offer: Option<String>,
    },
}

/// One `SETTING=VALUE` as the setting and its value, split at the first `=`, which no
/// setting's name holds.
fn assigned(given: &str) -> Result<(String, String), String> {
    given
        .split_once('=')
        .filter(|(setting, _)| !setting.is_empty())
        .map(|(setting, value)| (setting.to_owned(), value.to_owned()))
        .ok_or_else(|| format!("{given} is not SETTING=VALUE"))
}
