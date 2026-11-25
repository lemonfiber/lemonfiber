//! The documents a plugin author reads from this binary, answered without a stack.
//!
//! Three documents generated at build time from lemonfiber's own types, so the answer
//! is the same on a machine with nothing installed as on one running everything. Each
//! carries its own version rather than an envelope's `api_version`, because a plugin
//! author's editor and CI read the published file itself. Each is asked through the
//! dispatcher, as everything an author asks of a plugin's own source is.

use lemonfiber::cli::Authoring;
use lemonfiber_core::app::authoring::{Asked, Document};
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_error::Problem;

use crate::render;
use crate::translate::Under;

pub(crate) mod published;

pub(crate) async fn published(
    ctx: &Ctx,
    read: Document,
    json: bool,
) -> Result<render::Lines, Box<Problem>> {
    let octets = dispatch(Command::Authoring(Asked::Published(read)), ctx)
        .await
        .and_then(Outcome::octets)?;
    let text = String::from_utf8_lossy(&octets.bytes);
    Ok(shown(read, &text, json))
}

fn shown(read: Document, text: &str, json: bool) -> render::Lines {
    let viewed = match read {
        Document::Schema => None,
        _ if json => None,
        Document::Points => serde_json::from_str(text)
            .ok()
            .map(|points| render::plugin::points(&points)),
        Document::Vocabulary => serde_json::from_str(text)
            .ok()
            .map(|vocabulary| render::plugin::capabilities(&vocabulary)),
    };
    viewed.unwrap_or_else(|| render::plugin::document(text))
}

pub(crate) fn authored(read: Authoring) -> Under {
    let asked = match read {
        Authoring::Schema => return Under::Published(Document::Schema),
        Authoring::ExtensionPoints => return Under::Published(Document::Points),
        Authoring::Capabilities => return Under::Published(Document::Vocabulary),
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
