//! What reading the logs came to.

use lemonfiber_error::plural::s;

use super::Lines;

/// How many lines reading the logs came to, or that there were none.
pub(super) fn read(lines: u64) -> Lines {
    let mut said = Lines::default();
    if lines == 0 {
        said.put("no output");
    } else {
        said.put(format!(
            "{lines} line{} read",
            s(usize::try_from(lines).unwrap_or(usize::MAX))
        ));
    }
    said
}
