//! The words that extend the stack: installing, updating, removing, proving and running a
//! plugin, browsing what the catalogue offers, and the documents an author reads.

use lemonfiber_core::app::{plugins, Command};

use super::Under;
use crate::exit::USAGE;
use crate::say::complain;
use lemonfiber::cli::PluginCommand;

/// Which door this word goes through, and the value it goes through it as.
///
/// The three published documents answer the same on a machine with nothing installed as
/// on one running everything, and are printed as they were published; every other word
/// is a command and goes where every other verb goes.
///
/// # Errors
///
/// Where a value for a recipe's input is written without its `=`, said without
/// repeating what was written, since what follows a name may be a secret.
pub(crate) fn plugin(read: PluginCommand) -> Result<Under, u8> {
    let asked = match read {
        PluginCommand::Install {
            source,
            offer,
            approved,
            inputs,
        } => plugins::Asked::Install {
            source: lemonfiber_core::plugin::Source::named(&source),
            consent: consented(offer, approved, &inputs)?,
        },
        PluginCommand::Installed => plugins::Asked::Installed,
        PluginCommand::Remove { plugin, offer } => plugins::Asked::Remove {
            plugin,
            consent: consented(offer, Vec::new(), &[])?,
        },
        PluginCommand::Prove { plugin, offer } => plugins::Asked::Prove {
            plugin,
            consent: consented(offer, Vec::new(), &[])?,
        },
        PluginCommand::Run {
            plugin,
            recipe,
            offer,
            approved,
            inputs,
        } => plugins::Asked::Run {
            plugin,
            recipe,
            consent: consented(offer, approved, &inputs)?,
        },
        PluginCommand::Update {
            plugin,
            source,
            offer,
            approved,
            inputs,
        } => plugins::Asked::Update {
            plugin,
            source: lemonfiber_core::plugin::Source::named(&source),
            consent: consented(offer, approved, &inputs)?,
        },
        PluginCommand::Authoring(read) => return Ok(crate::authoring::authored(read)),
        PluginCommand::Catalogue => return Ok(Under::Dispatched(Command::PluginCatalogue)),
    };
    Ok(Under::Dispatched(Command::Plugins(asked)))
}

/// The yes an act on a plugin was given: the offer it answers, what it approves, and
/// the values given for its recipes' inputs, refused as [`given`] refuses.
fn consented(
    agreement: Option<String>,
    approved: Vec<String>,
    inputs: &[String],
) -> Result<plugins::Consent, u8> {
    Ok(plugins::Consent {
        agreement,
        approved,
        inputs: given(inputs)?,
    })
}

/// The values given for recipes' inputs with `--input`.
///
/// # Errors
///
/// Where one is written without its `=`, said without repeating it.
fn given(written: &[String]) -> Result<plugins::Inputs, u8> {
    plugins::Inputs::written(written).ok_or_else(|| {
        complain!(
            "error: an --input is written as the input's name, `=`, and its value, and one \
             was written without the `=`"
        );
        USAGE
    })
}
