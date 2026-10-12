use clap::Subcommand;

/// Changing one setting a service reads.
#[derive(Debug, Subcommand)]
pub enum SettingsCommand {
    /// Record a value for one setting, regenerating every entry that reads it.
    ///
    /// Named on its own it says what the change would come to — the entries it
    /// regenerates and the running services it restarts — changes nothing, and prints a
    /// name for that offer; answering with that name is the yes.
    Set {
        /// The setting, such as `gluetun:countries`.
        setting: String,
        /// The value, written exactly as given.
        value: String,
        /// The offer being answered, as the run that made it printed it.
        #[arg(long, value_name = "NAME")]
        offer: Option<String>,
    },
    /// Take your value away, so the declared default is written again.
    ///
    /// Named on its own it says what the change would come to and prints a name for that
    /// offer, as `set` does.
    Clear {
        /// The setting, such as `gluetun:countries`.
        setting: String,
        /// The offer being answered, as the run that made it printed it.
        #[arg(long, value_name = "NAME")]
        offer: Option<String>,
    },
}
