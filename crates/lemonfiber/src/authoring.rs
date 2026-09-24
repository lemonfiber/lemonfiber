//! What a plugin author asks this binary, answered without a stack.
//!
//! Five reads and no verbs. Three are documents generated at build time from
//! lemonfiber's own types, so the answer is the same on a machine with nothing
//! installed as on one running everything — which is the whole of what an author
//! needs them to be. The fourth reads a manifest the author is writing. The fifth
//! asks each image's registry what it holds beside that image, and is the only one
//! here that reaches the network.
//!
//! Kept out of the dispatcher because none of it dispatches: there is no stack to
//! ask, nothing to decide, and a context to build would be a context nothing reached
//! through.
//!
//! **Every read returns what it would say rather than saying it.** What to show and
//! what to exit with is a decision, and a decision returned as a value is one a test
//! can hold; putting it in front of an operator is the edge's, and the edge is the
//! one place a test cannot follow. The registry arrives the same way, so the read
//! that reaches the network is driven here without one.

use std::path::{Path, PathBuf};

use lemonfiber::cli::Authoring;
use lemonfiber_core::plugin::{Capabilities, Key, Ungenerated};
use lemonfiber_core::ports::registry::Registry;

use crate::exit::FAILURE;
use crate::render;

/// What one of these reads comes to: what to say, and what to end with.
pub(crate) struct Answered {
    /// What the operator is shown, where the read has an answer.
    pub(crate) lines: Option<render::Lines>,
    /// What went wrong, where something did.
    pub(crate) fault: Option<String>,
    /// The code this run ends with.
    pub(crate) code: u8,
}

impl Answered {
    /// An answer, and a run that ends well.
    fn shown(lines: render::Lines) -> Self {
        Self {
            lines: Some(lines),
            fault: None,
            code: 0,
        }
    }

    /// An answer that is itself the bad news, so it is shown and still fails.
    ///
    /// The author's CI is asking whether this plugin would be installed, and the
    /// whole of why it would not is in the lines. Exiting non-zero without them
    /// would answer the question and withhold the reason.
    fn refused(lines: render::Lines) -> Self {
        Self {
            lines: Some(lines),
            fault: None,
            code: FAILURE,
        }
    }

    /// Nothing could be answered, and this is why.
    fn faulted(fault: String) -> Self {
        Self {
            lines: None,
            fault: Some(fault),
            code: FAILURE,
        }
    }

    /// A document this build could not write, which is this build's fault.
    fn unwritable() -> Self {
        Self::faulted("the published document could not be written".to_owned())
    }
}

/// Answer one of the five things a plugin author asks this binary.
///
/// The schema is always the document, because it is a thing an editor reads rather
/// than a listing a person does. The other four have a form for each.
pub(crate) async fn published(read: &Authoring, json: bool, asking: &dyn Registry) -> Answered {
    match read {
        Authoring::Schema => document_of(lemonfiber_core::plugin::schema().as_deref()),
        Authoring::ExtensionPoints if json => {
            document_of(lemonfiber_core::plugin::points().as_deref())
        }
        Authoring::ExtensionPoints => Answered::shown(render::plugin::points(
            &lemonfiber_core::plugin::extension_points(),
        )),
        Authoring::Capabilities => capabilities(json),
        Authoring::Claims { path } => claims(path, json),
        Authoring::Provenance { path, keys } => vouched_for(path, keys, json, asking).await,
    }
}

/// One committed document, going out exactly as it was written.
fn document_of(text: Option<&str>) -> Answered {
    text.map(render::plugin::document)
        .map_or_else(Answered::unwritable, Answered::shown)
}

/// The capability vocabulary, in whichever form was asked for.
///
/// Its own function because it is the one of the three documents that can refuse: it
/// is read against the stack this build pins, and a stack that disagreed with the
/// vocabulary would have failed generation long before here. Reported as this build's
/// own fault rather than the operator's, because it is.
fn capabilities(json: bool) -> Answered {
    vocabulary(&lemonfiber_core::plugin::capabilities(), json)
}

/// The vocabulary in whichever form, from whatever generating it came to.
///
/// Takes the outcome rather than asking for it, so the refusal is a case a test can
/// put in front of this. Generation fails long before an operator asks for the
/// document, and the sentence it gives them is still the whole of what they get.
fn vocabulary(published: &Result<Capabilities, Ungenerated>, json: bool) -> Answered {
    match published {
        Ok(_) if json => document_of(lemonfiber_core::plugin::vocabulary().ok().as_deref()),
        Ok(published) => Answered::shown(render::plugin::capabilities(published)),
        Err(problem) => Answered::faulted(problem.to_string()),
    }
}

/// What one plugin's source claims, held to what this build publishes.
///
/// The one request under this word that reads something the operator has rather than
/// something lemonfiber published, and one of the two that can come back with an exit
/// status worth branching on: a manifest the vocabularies refuse, or a claim its own
/// recordings refute, is a plugin that would not be installed — which is what an
/// author's CI is asking.
///
/// Nothing is written and no service is asked anything. Every verdict is against the
/// recordings the plugin ships, which is what lets this run in a checkout with no
/// stack and no instance of the software anywhere.
fn claims(path: &Path, json: bool) -> Answered {
    let read = match lemonfiber_core::plugin::claimed(path) {
        Ok(read) => read,
        Err(unreadable) => return Answered::faulted(unreadable.to_string()),
    };
    render::plugin::claimed(&read, json).map_or_else(unrenderable, |lines| {
        if read.installable {
            Answered::shown(lines)
        } else {
            Answered::refused(lines)
        }
    })
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
async fn vouched_for(path: &Path, keys: &[PathBuf], json: bool, asking: &dyn Registry) -> Answered {
    let manifest = match lemonfiber_core::plugin::read(path) {
        Ok(manifest) => manifest,
        Err(unreadable) => return Answered::faulted(unreadable.to_string()),
    };
    let held = match held_keys(keys) {
        Ok(held) => held,
        Err(fault) => return Answered::faulted(fault),
    };
    let read = lemonfiber_core::plugin::vouched(&manifest, &held, asking).await;
    render::plugin::vouched(&read, json).map_or_else(unrenderable, |lines| {
        if read.installable {
            Answered::shown(lines)
        } else {
            Answered::refused(lines)
        }
    })
}

/// What was read could not be written as JSON, which is this build's fault.
fn unrenderable() -> Answered {
    Answered::faulted("what was read could not be written as JSON".to_owned())
}

/// The keys an operator named, read from the paths they gave.
///
/// A key that cannot be read stops the run rather than being dropped: a verification
/// carried out against fewer keys than were asked for would report *unproven* about an
/// image somebody did sign, and an operator who supplied a key is owed the news that
/// it was not used.
///
/// # Errors
///
/// The first path that could not be read or did not carry a key, said as the operator
/// would want to read it.
fn held_keys(paths: &[PathBuf]) -> Result<Vec<Key>, String> {
    let mut held = Vec::new();
    for path in paths {
        let named = path.display().to_string();
        let text = std::fs::read_to_string(path)
            .map_err(|why| format!("{named} could not be read: {why}"))?;
        held.push(Key::from_pem(&named, &text).map_err(|unusable| unusable.to_string())?);
    }
    Ok(held)
}

#[cfg(test)]
mod tests;
