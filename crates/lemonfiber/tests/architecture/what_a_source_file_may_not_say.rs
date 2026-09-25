//! The one thing a source file may not say that is not a comment.
//!
//! A suppression turns a rule the whole tree is held to into one that applies
//! wherever nobody objected. It is an attribute rather than a comment, which is why
//! it is here and not with the comment policy next door: the comment rules read
//! tokens a lexer classifies, and this reads the attributes the compiler reads.
//!
//! The omission is the point. The comment policy exempts machine directives from its
//! shape rules, and a suppression is not on that list — an exemption there would
//! hide a lint finding in the one place the comment gate has been told not to look.

use syn::visit::Visit;
use syn::Attribute;

use crate::source_tree::parsed;

/// The lints a file's attributes suppress, wherever in the file they sit.
fn suppressed(file: &syn::File) -> usize {
    struct Suppressing(usize);
    impl<'ast> Visit<'ast> for Suppressing {
        fn visit_attribute(&mut self, attribute: &'ast Attribute) {
            let path = attribute.path();
            let conditional = path.is_ident("cfg_attr")
                && attribute
                    .meta
                    .require_list()
                    .is_ok_and(|list| list.tokens.to_string().contains("allow"));
            if path.is_ident("allow") || conditional {
                self.0 += 1;
            }
        }
    }
    let mut suppressing = Suppressing(0);
    suppressing.visit_file(file);
    suppressing.0
}

/// Suppressions are not how a rule gets satisfied.
///
/// An allow attribute turns a standard the whole codebase is held to into one
/// that applies wherever nobody objected. Change the code, or change the rule
/// for everyone in the workspace manifest.
#[test]
fn no_lint_is_suppressed_in_source() {
    let suppressing: Vec<String> = parsed("crates")
        .iter()
        .filter(|(path, _)| !path.components().any(|part| part.as_os_str() == "tests"))
        .filter(|(path, _)| !path.ends_with("tests.rs"))
        .filter(|(_, file)| suppressed(file) > 0)
        .map(|(path, file)| format!("{} ({})", path.display(), suppressed(file)))
        .collect();
    assert!(
        suppressing.is_empty(),
        "these suppress a lint — change the code, or change the rule for everyone: \
         {suppressing:?}"
    );
}
