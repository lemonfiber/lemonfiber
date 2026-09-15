//! What a plugin author asks this binary, and what each answer costs to produce.
//!
//! Its own file rather than four functions among the dispatch, because none of them is
//! dispatch: nothing here reaches the core's one entry point, takes a context, or
//! touches a stack. Three are reads of documents this build generated at compile time,
//! and the fourth reads a manifest on a path — all four with no network, no catalogue
//! and nothing running.

use std::path::Path;
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
pub(crate) fn published(read: &PluginCommand, json: bool) -> ExitCode {
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
    if json {
        match serde_json::to_string_pretty(&read) {
            Ok(text) => document(&text).print(),
            Err(unwritable) => {
                complain!("error: what was read could not be written as JSON: {unwritable}");
                return ExitCode::from(FAILURE);
            }
        }
    } else {
        render::plugin::claims(&read).print();
    }
    if read.installable {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(FAILURE)
    }
}

/// The capability vocabulary, in whichever form was asked for.
///
/// Its own function because it is the one of the three documents that can refuse: it is
/// read
/// against the stack this build pins, and a stack that disagreed with the vocabulary
/// would have failed generation long before here. Reported as this build's own fault
/// rather than the operator's, because it is.
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
