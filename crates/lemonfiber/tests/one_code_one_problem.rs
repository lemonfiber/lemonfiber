//! One number, one problem, for as long as the number exists.
//!
//! A code is the token an operator searches for, and searching is the whole of what
//! it is for: a number answering for two problems sends whoever typed it to the
//! wrong page, and it does so most often on the day two branches each took the next
//! free number without seeing the other. That is not a hypothetical shape of
//! mistake here, which is why this reads the declarations rather than the reference
//! generated from them.
//!
//! Its own file because it is the only rule about the inventory of codes. What that
//! inventory is rendered into, and whether the committed artefact still matches it,
//! is held beside the renderer.

use std::collections::BTreeMap;
use std::path::PathBuf;

mod source_tree;

use source_tree::workspace_root;

/// No two problems answer to the same code.
///
/// The error model deliberately declares each code as a `const` beside the error
/// that raises it rather than in one shared enum, so that adding an error is not
/// editing a list everyone edits. What that decision costs is the one property a
/// central list would have given for free: nothing stops two errors picking the
/// same string.
///
/// It costs something real. An operator who searches for a code should find the
/// same answer a year later, and for four releases `VPN-5` was both a port
/// mismatch and a killswitch leak — so whoever looked one up found the other.
///
/// Read through the same reader that writes the committed inventory, so that what
/// counts as a declaration is decided once. A second reader would answer this
/// question about a slightly different set of declarations than the one the
/// artefact is built from, and the two would drift without either being wrong.
#[test]
fn no_two_problems_answer_to_the_same_code() {
    let Ok(declared) = lemonfiber::codes::declared(&workspace_root()) else {
        unreachable!("the workspace these tests run in is readable");
    };

    let mut seen: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut collisions: Vec<String> = Vec::new();
    for (path, code) in declared {
        if let Some(first) = seen.insert(code.clone(), path.clone()) {
            collisions.push(format!(
                "{code} in {} and {}",
                first.display(),
                path.display()
            ));
        }
    }
    assert!(collisions.is_empty(), "{collisions:?}");
}
