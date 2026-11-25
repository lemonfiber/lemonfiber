//! What reading the logs came to, and where an export of them went.

use lemonfiber_core::app::exporting::Exported;
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

/// Where what the log viewer showed was written, or would have been.
pub(crate) fn exported(report: &Exported) -> Lines {
    let mut said = Lines::default();
    if report.rehearsed {
        said.put(format!("would be written to {}", report.path));
    } else {
        said.put(format!("written to {}", report.path));
    }
    said
}

#[cfg(test)]
mod tests;
