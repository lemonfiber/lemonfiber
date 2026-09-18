//! What a plugin author asks this binary, answered without a stack.
//!
//! Four requests and no context between them. Three are documents generated at build
//! time from lemonfiber's own types, so the answer is the same on a machine with
//! nothing installed as on one running everything — which is the whole of what an
//! author needs them to be. The fourth reads a manifest the author is writing, and
//! the fifth is the only one here that reaches the network.
//!
//! Kept out of the dispatcher because none of it dispatches: there is no stack to
//! ask, nothing to decide, and a context to build would be a context nothing reached
//! through.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use lemonfiber::cli::PluginCommand;

use crate::exit::FAILURE;
use crate::render;
use crate::say::complain;

/// Answer one of the four things a plugin author asks this binary.
///
/// No context, no dispatch and no stack. Three are documents generated at build time
/// from lemonfiber's own types, so the answer is the same on a machine with nothing
/// installed as on one running everything — which is the whole of what an author needs
/// them to be. The fourth reads a manifest the author is writing.
///
/// The schema is always the document, because it is a thing an editor reads rather than
/// a listing a person does. The other three have a form for each.
pub(crate) async fn published(read: &PluginCommand, json: bool) -> ExitCode {
    let lines = match read {
        PluginCommand::Schema => lemonfiber_core::plugin::schema().as_deref().map(document),
        PluginCommand::ExtensionPoints if json => {
            lemonfiber_core::plugin::points().as_deref().map(document)
        }
        PluginCommand::ExtensionPoints => Some(render::plugin::points(
            &lemonfiber_core::plugin::extension_points(),
        )),
        PluginCommand::Capabilities => match capabilities(json) {
            Ok(lines) => lines,
            Err(code) => return code,
        },
        PluginCommand::Claims { path } => return claims(path, json),
        PluginCommand::Provenance { path, keys } => return vouched_for(path, keys, json).await,
    };
    let Some(lines) = lines else {
        complain!("error: the published document could not be written");
        return ExitCode::from(FAILURE);
    };
    lines.print();
    ExitCode::SUCCESS
}

/// What one plugin's source claims, held to what this build publishes.
///
/// The one request under this word that reads something the operator has rather than
/// something lemonfiber published, and the one that can come back with an exit status
/// worth branching on: a manifest the vocabularies refuse, or a claim its own
/// recordings refute, is a plugin that would not be installed — which is what an
/// author's CI is asking.
///
/// Nothing is written and no service is asked anything. Every verdict is against the
/// recordings the plugin ships, which is what lets this run in a checkout with no stack
/// and no instance of the software anywhere.
fn claims(path: &Path, json: bool) -> ExitCode {
    let read = match lemonfiber_core::plugin::claimed(path) {
        Ok(read) => read,
        Err(unreadable) => {
            complain!("error: {unreadable}");
            return ExitCode::from(FAILURE);
        }
    };
    let Some(lines) = render::plugin::claimed(&read, json) else {
        complain!("error: what was read could not be written as JSON");
        return ExitCode::from(FAILURE);
    };
    lines.print();
    if read.installable {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(FAILURE)
    }
}

/// What anybody has said about the images a plugin pins.
///
/// The one request under this word that reaches the network, and it reaches exactly
/// one kind of place: the registry each image is already pinned in. Nothing is
/// installed, nothing is written, and no service is asked anything.
///
/// A refused image exits non-zero, which is what a catalogue's CI is asking. An
/// unproven one does not: refusing every image nobody signed would refuse most of the
/// registry, and the answer an operator needs is which of the three it is rather than
/// a pass or a fail.
async fn vouched_for(path: &Path, keys: &[PathBuf], json: bool) -> ExitCode {
    let manifest = match lemonfiber_core::plugin::read(path) {
        Ok(manifest) => manifest,
        Err(unreadable) => {
            complain!("error: {unreadable}");
            return ExitCode::from(FAILURE);
        }
    };
    let held = match held_keys(keys) {
        Ok(held) => held,
        Err(code) => return code,
    };
    let asking = lemonfiber_adapters::registry::Oci::new(lemonfiber_adapters::http::Web::new());
    let read = lemonfiber_core::plugin::vouched(&manifest, &held, &asking).await;
    let Some(lines) = render::plugin::vouched(&read, json) else {
        complain!("error: what was read could not be written as JSON");
        return ExitCode::from(FAILURE);
    };
    lines.print();
    if read.installable {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(FAILURE)
    }
}

/// The keys an operator named, read from the paths they gave.
///
/// A key that cannot be read stops the run rather than being dropped: a verification
/// carried out against fewer keys than were asked for would report *unproven* about an
/// image somebody did sign, and an operator who supplied a key is owed the news that
/// it was not used.
fn held_keys(paths: &[PathBuf]) -> Result<Vec<lemonfiber_core::plugin::Key>, ExitCode> {
    let mut held = Vec::new();
    for path in paths {
        let named = path.display().to_string();
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(why) => {
                complain!("error: {named} could not be read: {why}");
                return Err(ExitCode::from(FAILURE));
            }
        };
        match lemonfiber_core::plugin::Key::from_pem(&named, &text) {
            Ok(key) => held.push(key),
            Err(unusable) => {
                complain!("error: {unusable}");
                return Err(ExitCode::from(FAILURE));
            }
        }
    }
    Ok(held)
}

/// The capability vocabulary, in whichever form was asked for.
///
/// Its own function because it is the one of the three documents that can refuse: it
/// is read against the stack this build pins, and a stack that disagreed with the
/// vocabulary would have failed generation long before here. Reported as this build's
/// own fault rather than the operator's, because it is.
fn capabilities(json: bool) -> Result<Option<render::Lines>, ExitCode> {
    match lemonfiber_core::plugin::capabilities() {
        Ok(_) if json => Ok(lemonfiber_core::plugin::vocabulary()
            .ok()
            .as_deref()
            .map(document)),
        Ok(published) => Ok(Some(render::plugin::capabilities(&published))),
        Err(problem) => {
            complain!("error: {problem}");
            Err(ExitCode::from(FAILURE))
        }
    }
}

/// One committed document, going out exactly as it was written.
fn document(text: &str) -> render::Lines {
    render::plugin::document(text)
}
