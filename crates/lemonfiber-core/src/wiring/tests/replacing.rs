//! Which parts a choice of filler takes out of their form.

use super::{service, stack, written, Chosen};
use crate::wiring::replaced;

/// A stack where `asker` asks one media server of the two the stack and a plugin bring.
fn two_servers() -> Option<lemonfiber_manifest::Manifest> {
    written(&format!(
        "{}{}\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\n",
        service("asker", ""),
        service("jelly", "\"media.serve\"")
    ))
}

/// A plugin bringing a media server behind its adapter.
fn a_fronted_server() -> crate::plugin::Installed {
    let mut adapter = crate::test_support::a_placed("server-adapter", &["media.serve"], Some(8080));
    adapter.speaks = vec!["media.serve@1".to_owned()];
    adapter.fronts = Some("server".to_owned());
    let upstream = crate::test_support::a_placed("server", &[], Some(8096));
    crate::test_support::an_installed("serving", vec![adapter, upstream])
}

fn out(
    manifest: Option<lemonfiber_manifest::Manifest>,
    installed: &[crate::plugin::Installed],
    chosen: &str,
) -> Option<Vec<String>> {
    manifest.map(|manifest| replaced(&manifest, installed, &Chosen::read(Some(chosen))))
}

#[test]
fn nothing_chosen_takes_nothing_out_of_its_form() {
    assert_eq!(
        stack().map(|manifest| replaced(&manifest, &[], &Chosen::default())),
        Some(Vec::new())
    );
}

#[test]
fn a_part_a_choice_passed_over_and_that_fills_nothing_else_is_taken_out() {
    assert_eq!(
        out(stack(), &[], "indexer.search=nzbhydra2"),
        Some(vec!["prowlarr".to_owned()])
    );
}

#[test]
fn a_part_still_filling_another_ask_stays_in_its_form() {
    let manifest = written(&format!(
        "{}{}{}\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\n\
         \n[[wiring]]\nby = \"asker\"\nasks = \"identity.source\"\n",
        service("asker", ""),
        service("both", "\"media.serve\", \"identity.source\""),
        service("other", "\"media.serve\"")
    ));
    assert_eq!(out(manifest, &[], "media.serve=other"), Some(Vec::new()));
}

#[test]
fn an_adapter_passed_over_takes_the_upstream_it_fronts_out_with_it() {
    let installed = [a_fronted_server()];
    assert_eq!(
        out(two_servers(), &installed, "media.serve=jelly"),
        Some(vec!["server-adapter".to_owned(), "server".to_owned()])
    );
    assert_eq!(
        out(two_servers(), &installed, "media.serve=server-adapter"),
        Some(vec!["jelly".to_owned()])
    );
}
