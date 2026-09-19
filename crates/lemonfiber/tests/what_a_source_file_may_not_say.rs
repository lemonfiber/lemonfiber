//! The one thing a source file may not say that is not a comment.
//!
//! A suppression turns a rule the whole tree is held to into one that applies
//! wherever nobody objected. It is an attribute rather than a comment, which is why
//! it is here and not with the comment policy next door: the comment rules read
//! tokens a lexer classifies, and this reads a line of code.
//!
//! The omission is the point. The comment policy exempts machine directives from its
//! shape rules, and a suppression is not on that list — an exemption there would
//! hide a lint finding in the one place the comment gate has been told not to look.

mod source_tree;

use source_tree::sources;

/// Suppressions are not how a rule gets satisfied.
///
/// An allow attribute turns a standard the whole codebase is held to into one
/// that applies wherever nobody objected. Change the code, or change the rule
/// for everyone in the workspace manifest.
#[test]
fn no_lint_is_suppressed_in_source() {
    for (path, text) in sources() {
        if path.starts_with("crates") && path.to_string_lossy().contains("tests") {
            continue;
        }
        for (number, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            assert!(
                !(trimmed.starts_with("#[allow(") || trimmed.starts_with("#![allow(")),
                "{}:{} suppresses a lint — change the code, or change the rule for everyone",
                path.display(),
                number + 1
            );
        }
    }
}
