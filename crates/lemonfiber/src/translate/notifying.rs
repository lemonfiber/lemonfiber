//! What was asked of the back-ends, as the operator at this machine's terminal asks it.

use lemonfiber::cli::{NotifyCommand, NotifyKind, NotifyRetries, NotifySecrets, RawNotify};
use lemonfiber_core::app::Command;
use lemonfiber_core::model::BackendKind;
use lemonfiber_core::notify::configuring::{Adding, Asked, Offering, Setting};

/// What was asked of the back-ends, each secret read from the variable named through
/// `environment`; a variable named that holds nothing is handed on as nothing given.
pub(crate) fn notifying(asked: RawNotify, environment: &dyn Fn(&str) -> Option<String>) -> Command {
    let secret =
        |variable: Option<String>| variable.map(|name| environment(&name).unwrap_or_default());
    Command::Notify(match asked.action {
        NotifyCommand::List => Asked::List,
        NotifyCommand::Add {
            id,
            kind,
            url,
            topic,
            secrets:
                NotifySecrets {
                    token_env,
                    reader_env,
                },
            hosted,
            retries: NotifyRetries { attempts, delays },
        } => Asked::Add(Box::new(Adding {
            id,
            kind: spoken(kind),
            url,
            topic,
            token: secret(token_env),
            reader: secret(reader_env),
            hosted,
            attempts,
            delays,
        })),
        NotifyCommand::Set {
            id,
            secrets:
                NotifySecrets {
                    token_env,
                    reader_env,
                },
            retries: NotifyRetries { attempts, delays },
            members,
            no_members,
        } => Asked::Set(Box::new(Setting {
            id,
            token: secret(token_env),
            reader: secret(reader_env),
            attempts,
            delays,
            members: members
                .map(Offering::Offer)
                .or_else(|| no_members.then_some(Offering::Withdraw)),
        })),
        NotifyCommand::Test { id } => Asked::Test { id },
        NotifyCommand::Remove { id } => Asked::Remove { id },
    })
}

/// The kind a back-end named on the command line speaks.
const fn spoken(kind: NotifyKind) -> BackendKind {
    match kind {
        NotifyKind::Ntfy => BackendKind::Ntfy,
        NotifyKind::Gotify => BackendKind::Gotify,
        NotifyKind::Webhook => BackendKind::Webhook,
        NotifyKind::Apprise => BackendKind::Apprise,
    }
}

#[cfg(test)]
mod tests;
