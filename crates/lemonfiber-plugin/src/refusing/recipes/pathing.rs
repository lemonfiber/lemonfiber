//! A call's address is built when the manifest is read, the way the call builds it.
//!
//! Each step's address is built with [`crate::addressing::address`], the function the
//! call itself is made with, and whatever it will not build is refused here: a path
//! that is not a plain absolute path, with a code of its own, and a host an address
//! would carry spelled other than as the manifest writes it.

use crate::addressing::{address, Toward, Unaddressed};
use crate::schema::{Manifest, Step};
use crate::Violation;

use super::whither;

/// The port an address is built on where its destination publishes none, which is
/// refused beside this: what a path may be does not depend on the port it is sent to.
const UNPUBLISHED: u16 = 0;

/// What reading a manifest builds a step's address toward: where it goes, as reading it
/// classes it, and a service in the stack where it is neither, so its path is still read.
fn toward<'a>(manifest: &Manifest, step: &'a Step) -> Toward<'a> {
    whither::toward(manifest, &step.call.to).unwrap_or(Toward::Stack(UNPUBLISHED))
}

/// What building this step's address refuses, as reading the manifest builds it.
fn unaddressed(manifest: &Manifest, step: &Step) -> Option<Unaddressed> {
    address(&step.call.path, toward(manifest, step), &str::to_owned).err()
}

/// Refuse a step whose address the call could not build as the manifest writes it.
pub(super) fn plain(manifest: &Manifest, step: &Step, at: &str, found: &mut Vec<Violation>) {
    match unaddressed(manifest, step) {
        None => {}
        Some(Unaddressed::Path(why)) => found.push(Violation {
            location: format!("{at}.call.path"),
            message: format!(
                "{:?} is not a plain absolute path: {why}; a call's path begins with one `/` \
                 and is set on the address its destination gives it",
                step.call.path
            ),
        }),
        Some(Unaddressed::Host(why)) => found.push(Violation {
            location: format!("{at}.call.to"),
            message: format!(
                "{why}, so the call would not go to {} as written; a host is written the way \
                 an address carries it",
                step.call.to
            ),
        }),
    }
}

/// Whether any recipe of this manifest calls a path that is not plain, which is refused
/// with a code of its own.
#[must_use]
pub fn names_a_path_not_plain(manifest: &Manifest) -> bool {
    manifest
        .recipes
        .iter()
        .flat_map(|recipe| &recipe.steps)
        .any(|step| matches!(unaddressed(manifest, step), Some(Unaddressed::Path(_))))
}

#[cfg(test)]
mod tests;
