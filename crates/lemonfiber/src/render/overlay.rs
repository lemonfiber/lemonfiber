//! The operator's overlay, named wherever a run would start the project it is merged over.

use lemonfiber_core::model::Unvetted;

use super::Lines;

/// The overlay and every service it changes, said as unvetted, or nothing where there is
/// no overlay.
pub(crate) fn unvetted(overlay: Option<&Unvetted>) -> Lines {
    let mut lines = Lines::default();
    if let Some(overlay) = overlay {
        let changes = overlay.changes();
        lines.spaced(format!(
            "Unvetted: your overlay {} is merged over the project and {changes}; nothing in \
             it is held to what lemonfiber checks.",
            overlay.overlay
        ));
    }
    lines
}

#[cfg(test)]
mod tests;
