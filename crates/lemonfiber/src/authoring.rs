//! The documents a plugin author reads from this binary, answered without a stack.
//!
//! Three documents generated at build time from lemonfiber's own types, so the answer
//! is the same on a machine with nothing installed as on one running everything. Each
//! carries its own version rather than an envelope's `api_version`, because a plugin
//! author's editor and CI read the published file itself. What an author asks of a
//! plugin's own source is a command, and goes through the dispatcher.

use lemonfiber::cli::Authoring;
use lemonfiber_core::app::authoring::Asked;
use lemonfiber_core::app::Command;
use lemonfiber_core::plugin::{Capabilities, Ungenerated};

use crate::exit::FAILURE;
use crate::render;
use crate::translate::Under;

/// One of the documents this build publishes for a plugin author.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Document {
    /// The schema a manifest is validated against.
    Schema,
    /// Every extension point and the register each one fills.
    ExtensionPoints,
    /// The capability vocabulary.
    Capabilities,
}

/// What reading a document comes to: what to say, and what to end with.
pub(crate) struct Answered {
    /// What the author is shown, where the document was written.
    pub(crate) lines: Option<render::Lines>,
    /// What went wrong, where something did.
    pub(crate) fault: Option<String>,
    /// The code this run ends with.
    pub(crate) code: u8,
}

impl Answered {
    /// A document, and a run that ends well.
    fn shown(lines: render::Lines) -> Self {
        Self {
            lines: Some(lines),
            fault: None,
            code: 0,
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

/// One published document, as the document itself or as a listing for a person.
///
/// The schema is always the document, because an editor reads it rather than a person.
pub(crate) fn published(read: Document, json: bool) -> Answered {
    match read {
        Document::Schema => document_of(lemonfiber_core::plugin::schema().as_deref()),
        Document::ExtensionPoints if json => {
            document_of(lemonfiber_core::plugin::points().as_deref())
        }
        Document::ExtensionPoints => Answered::shown(render::plugin::points(
            &lemonfiber_core::plugin::extension_points(),
        )),
        Document::Capabilities => capabilities(json),
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

/// Which door a word an author asks goes through: a document is published, and a read
/// of the author's own source is a command.
pub(crate) fn authored(read: Authoring) -> Under {
    let asked = match read {
        Authoring::Schema => return Under::Published(Document::Schema),
        Authoring::ExtensionPoints => return Under::Published(Document::ExtensionPoints),
        Authoring::Capabilities => return Under::Published(Document::Capabilities),
        Authoring::Claims { path } => Asked::Claims { path },
        Authoring::Conform { path } => Asked::Conform { path },
        Authoring::Provenance { path, keys } => Asked::Provenance { path, keys },
        Authoring::Record {
            path,
            check,
            adapter,
            built,
        } => Asked::Record {
            path,
            taking: lemonfiber_core::plugin::Retaking {
                check,
                adapter,
                built,
            },
        },
    };
    Under::Dispatched(Command::Authoring(asked))
}

#[cfg(test)]
mod tests;
