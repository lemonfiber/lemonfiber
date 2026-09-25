//! What the coverage report can and cannot watch being decided.
//!
//! The rules beside it are about where a *name* may appear, and this is about where
//! a *decision* may sit, so it is a file of its own.

use syn::visit::Visit;

use crate::source_tree::parsed;

/// Nothing decides anything inside an `#[async_trait]` method body.
///
/// `#[async_trait]` rewrites a method body into a generated future, and the coverage
/// report attributes nothing inside it to the lines it came from. The signature carries
/// a count and every line beneath it carries none, so a branch in there can go untaken
/// for ever and the gate that says this workspace is fully covered will not say a word.
///
/// Shown before it was relied on: the same never-taken branch planted inside such a
/// method got no coverage region at all, while the identical branch in a plain `fn` and
/// in a plain `async fn` was mapped, counted zero and reported.
///
/// Only branching. Straight-line code loses nothing — if the method ran, those lines
/// ran, and the signature's count says so. And a closure inside the body is fine: it
/// compiles to a function item of its own, which the report does see. What is refused is
/// a bare `if`, `match`, `for`, `while`, `loop` or `return`, where a line can be skipped
/// while the method still runs and nothing anywhere reports it.
///
/// The remedy is never an exemption. Move the body to a plain `async fn` beside the
/// impl and delegate to it: the asynchrony is unchanged, and the decision lands
/// somewhere the gate can watch it being made.
#[test]
fn nothing_decides_inside_a_body_the_coverage_report_cannot_see() {
    let mut hidden: Vec<String> = Vec::new();
    let mut seen = 0_usize;

    for (path, file) in parsed("crates") {
        if path.components().any(|part| part.as_os_str() == "tests") || path.ends_with("tests.rs") {
            continue;
        }
        for method in async_trait_methods(&file) {
            seen += 1;
            if decides(&method.block) {
                hidden.push(format!("{}: {}", path.display(), method.sig.ident));
            }
        }
    }

    assert!(
        seen > 100,
        "the scan found {seen} bodies, which means it is looking in the wrong place"
    );
    assert!(
        hidden.is_empty(),
        "a decision here is one the coverage gate cannot watch being made; move the \
         body to a plain async fn and delegate: {}",
        hidden.join(", ")
    );
}

/// Every `async fn` in an `impl` block marked `#[async_trait]`, however deep.
fn async_trait_methods(file: &syn::File) -> Vec<syn::ImplItemFn> {
    #[derive(Default)]
    struct Methods(Vec<syn::ImplItemFn>);
    impl<'ast> Visit<'ast> for Methods {
        fn visit_item_impl(&mut self, block: &'ast syn::ItemImpl) {
            let marked = block
                .attrs
                .iter()
                .any(|attribute| attribute.path().is_ident("async_trait"));
            if marked {
                self.0
                    .extend(block.items.iter().filter_map(|item| match item {
                        syn::ImplItem::Fn(method) if method.sig.asyncness.is_some() => {
                            Some(method.clone())
                        }
                        _ => None,
                    }));
            }
            syn::visit::visit_item_impl(self, block);
        }
    }
    let mut methods = Methods::default();
    methods.visit_file(file);
    methods.0
}

/// Whether a body branches or returns anywhere outside a closure: a closure compiles
/// to a function of its own, which the report does see.
fn decides(body: &syn::Block) -> bool {
    #[derive(Default)]
    struct Deciding(bool);
    impl<'ast> Visit<'ast> for Deciding {
        fn visit_expr(&mut self, expression: &'ast syn::Expr) {
            match expression {
                syn::Expr::Closure(_) => {}
                syn::Expr::If(_)
                | syn::Expr::Match(_)
                | syn::Expr::ForLoop(_)
                | syn::Expr::While(_)
                | syn::Expr::Loop(_)
                | syn::Expr::Return(_) => self.0 = true,
                _ => syn::visit::visit_expr(self, expression),
            }
        }
    }
    let mut deciding = Deciding::default();
    deciding.visit_block(body);
    deciding.0
}
