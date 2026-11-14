use super::declared;
use crate::error::codes::plugin::UNDECLARED_FORM;
use crate::test_support::{an_installed, stack};

fn naming(forms: &[&str]) -> crate::plugin::Installed {
    crate::plugin::Installed {
        forms: forms.iter().map(|form| (*form).to_owned()).collect(),
        ..an_installed("comics", Vec::new())
    }
}

#[test]
fn a_plugin_naming_only_declared_forms_is_let_through() {
    let manifest = stack().manifest();
    assert!(manifest.is_ok());
    assert!(manifest.is_ok_and(|manifest| declared(&naming(&["books", "full"]), &manifest).is_ok()));
}

#[test]
fn a_plugin_naming_a_form_the_stack_does_not_declare_is_refused_naming_it() {
    let refused = stack()
        .manifest()
        .ok()
        .and_then(|manifest| declared(&naming(&["books", "comics", "cartoons"]), &manifest).err());
    assert_eq!(
        refused.as_ref().map(|problem| problem.code),
        Some(UNDECLARED_FORM)
    );
    assert!(
        refused.is_some_and(|problem| problem.summary.ends_with("comics, cartoons")
            && problem.meaning.contains("books")
            && !problem.summary.contains("books"))
    );
}
