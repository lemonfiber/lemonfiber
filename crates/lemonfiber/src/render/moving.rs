//! A move in place on offer, named wherever a run would start or update the stack.

use lemonfiber_core::app::plugins::MoveOffered;
use lemonfiber_core::update::run::Moved;

use super::Lines;

/// Each part this release runs as a plugin that the stack still runs itself, with the
/// reading that takes the move, or nothing where none is on offer.
pub(crate) fn offered(moves: &[MoveOffered]) -> Lines {
    let mut lines = Lines::default();
    for one in moves {
        lines.spaced(format!(
            "On offer: this release runs {} as a plugin, and it still runs as the stack's own \
             service. Moving it keeps its container, configuration, data and credentials; \
             `lemonfiber update stack` shows the move, and confirming the update takes it.",
            one.plugin
        ));
        if let Some(adapter) = &one.configured_by {
            lines.put(format!(
                "Until it moves, {} runs without {adapter}, which writes the configuration it \
                 reads, so it starts without what that holds, the credentials it reaches other \
                 services with included.",
                one.service
            ));
        }
    }
    lines
}

/// Each move in place a confirmed update took, or could not, with why.
pub(crate) fn moved(moved: &[Moved]) -> Lines {
    let mut lines = Lines::default();
    for one in moved {
        lines.spaced(match &one.why {
            None if one.taken => format!(
                "Moved: {} now runs as its plugin in the place of the stack's own {}.",
                one.plugin, one.service
            ),
            why => format!(
                "Not moved: {} still runs as the stack's own service{}.",
                one.service,
                why.as_ref()
                    .map(|why| format!(", because {why}"))
                    .unwrap_or_default()
            ),
        });
    }
    lines
}

#[cfg(test)]
mod tests;
