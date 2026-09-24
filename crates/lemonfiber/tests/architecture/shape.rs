//! What a piece of parsed code does: the functions it declares, the macros it
//! invokes, and the functions and methods it calls.
//!
//! The checks that ask about the shape of the code ask it here, of the syntax tree,
//! so a name in a comment or a string is never mistaken for a call.

use syn::visit::Visit;

/// The function `name` a file declares, at the top level or in an `impl`.
pub(crate) fn function(file: &syn::File, name: &str) -> Option<syn::Block> {
    file.items.iter().find_map(|item| match item {
        syn::Item::Fn(found) if found.sig.ident == name => Some((*found.block).clone()),
        syn::Item::Impl(block) => block.items.iter().find_map(|inner| match inner {
            syn::ImplItem::Fn(found) if found.sig.ident == name => Some(found.block.clone()),
            _ => None,
        }),
        _ => None,
    })
}

/// What a node reaches: each macro with the text of its arguments, and the last
/// segment of each function or method it calls.
#[derive(Default)]
pub(crate) struct Reached {
    pub(crate) macros: Vec<(String, String)>,
    pub(crate) calls: Vec<String>,
}

impl Reached {
    /// What a whole file reaches.
    pub(crate) fn in_file(file: &syn::File) -> Self {
        let mut reached = Self::default();
        reached.visit_file(file);
        reached
    }

    /// What one block reaches.
    pub(crate) fn in_block(block: &syn::Block) -> Self {
        let mut reached = Self::default();
        reached.visit_block(block);
        reached
    }

    /// The arguments of every invocation of the macro `name`.
    pub(crate) fn invoked(&self, name: &str) -> Vec<&str> {
        self.macros
            .iter()
            .filter(|(called, _)| called == name)
            .map(|(_, arguments)| arguments.as_str())
            .collect()
    }

    /// Whether anything calls a function or method named `name`.
    pub(crate) fn calls(&self, name: &str) -> bool {
        self.calls.iter().any(|called| called == name)
    }
}

impl<'ast> Visit<'ast> for Reached {
    fn visit_macro(&mut self, called: &'ast syn::Macro) {
        if let Some(last) = called.path.segments.last() {
            self.macros
                .push((last.ident.to_string(), called.tokens.to_string()));
        }
        // A macro's arguments are tokens rather than syntax, so the calls inside
        // one are read from them where they parse as expressions.
        if let Ok(arguments) = called.parse_body_with(
            syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated,
        ) {
            for argument in &arguments {
                self.visit_expr(argument);
            }
        }
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(called) = call.func.as_ref() {
            if let Some(last) = called.path.segments.last() {
                self.calls.push(last.ident.to_string());
            }
        }
        syn::visit::visit_expr_call(self, call);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        self.calls.push(call.method.to_string());
        syn::visit::visit_expr_method_call(self, call);
    }
}
