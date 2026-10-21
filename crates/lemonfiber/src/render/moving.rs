//! A move in place on offer, named wherever a run would start or update the stack.

use lemonfiber_core::app::plugins::MoveOffered;

use super::Lines;

/// Each part this release runs as a plugin that the stack still runs itself, with the
/// reading that takes the move, or nothing where none is on offer.
pub(crate) fn offered(moves: &[MoveOffered]) -> Lines {
    let mut lines = Lines::default();
    for one in moves {
        lines.spaced(format!(
            "On offer: this release runs {} as a plugin, and it still runs as the stack's own \
             service. Moving it keeps its container, configuration, data and credentials; read \
             the move with `lemonfiber plugin install {}`.",
            one.service, one.plugin
        ));
    }
    lines
}

#[cfg(test)]
mod tests;
