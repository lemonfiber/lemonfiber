//! What an operation takes away, and whether anybody is told before it happens.
//!
//! A check that disturbs the stack says how long for, and `what_a_check_can_see.rs`
//! holds that half. This is the other: the verbs that start and stop services. The
//! two halves are read from different places — one from the shape of a check, one
//! from the shape of the dispatcher and the table that describes every command.

use syn::visit::Visit;
use syn::{Expr, ExprMatch, Pat, Stmt};

use crate::shape;
use crate::source_tree::parsed;

/// The body of the function `name` in the file at `path`, as the compiler reads it.
fn body(path: &str, name: &str) -> Option<syn::Block> {
    parsed(path)
        .iter()
        .find_map(|(_, file)| shape::function(file, name))
}

/// Whether a statement calls a function whose path ends in `last`, anywhere in it.
fn calls(statement: &Stmt, last: &str) -> bool {
    struct Calling<'a> {
        last: &'a str,
        found: bool,
    }
    impl<'ast> Visit<'ast> for Calling<'_> {
        fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
            if let Expr::Path(called) = call.func.as_ref() {
                self.found |= called
                    .path
                    .segments
                    .last()
                    .is_some_and(|segment| segment.ident == self.last);
            }
            syn::visit::visit_expr_call(self, call);
        }
    }
    let mut calling = Calling { last, found: false };
    calling.visit_stmt(statement);
    calling.found
}

/// Every command says what it takes away before the arm that takes it runs.
///
/// Read from the dispatcher rather than from the arms, because that is where it has
/// to happen: an operation that stated its own cost is an operation somebody can add
/// without stating one, and the order matters as much as the call — a length said
/// after the services have stopped is a length nobody had a use for.
#[test]
fn every_command_says_what_it_disturbs_before_it_disturbs_it() {
    let Some(dispatch) = body("crates/lemonfiber-core/src/app.rs", "dispatch") else {
        unreachable!("the core has one entry point, and it is `app::dispatch`");
    };
    let statements = &dispatch.stmts;
    let said = statements
        .iter()
        .position(|statement| calls(statement, "said"));
    let routed = statements
        .iter()
        .position(|statement| calls(statement, "routed"));

    assert!(
        said.is_some(),
        "the dispatcher no longer says what a command takes away, so no surface does"
    );
    assert!(
        said < routed,
        "what a command takes away is said after it is taken, which is a length \
         nobody had a use for"
    );
}

/// What each command disturbs is decided arm by arm, with no arm for the rest.
///
/// A wildcard would answer *disturbs nothing* for every command nobody has thought
/// about yet, and a machine taken away in silence is the failure this exists to
/// prevent; without one, a new command stops the build until somebody has decided.
#[test]
fn what_a_command_disturbs_is_answered_without_a_wildcard() {
    let Some(asked) = body("crates/lemonfiber-core/src/app/rehearsal.rs", "asked") else {
        unreachable!("every command is described by `rehearsal::asked`");
    };
    let arms: Vec<&Pat> = asked
        .stmts
        .iter()
        .filter_map(|statement| match statement {
            Stmt::Expr(Expr::Match(ExprMatch { arms, .. }), _) => Some(arms),
            _ => None,
        })
        .flatten()
        .map(|arm| &arm.pat)
        .collect();

    assert!(
        !arms.is_empty(),
        "the table describing every command was not read, so this rule read nothing"
    );
    assert!(
        !arms.iter().any(|pat| matches!(pat, Pat::Wild(_))),
        "a wildcard here answers for every command nobody has thought about yet"
    );
}
