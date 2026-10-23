use super::{parse, whole, WHOLE};

#[test]
fn a_wiring_naming_no_dashboard_line_or_icon_leaves_the_plugins_description_and_no_icon() {
    let manifest = whole();
    let entry = manifest
        .as_ref()
        .and_then(|one| one.services.first().map(|service| one.entry(service)));
    assert_eq!(
        entry.map(|entry| entry.description),
        Some("Reads your comics on any browser")
    );
    assert_eq!(entry.map(|entry| entry.icon), Some(None));
    assert_eq!(entry.map(|entry| entry.hidden), Some(false));
}

#[test]
fn a_wiring_hiding_its_entry_is_read_as_hidden() {
    let hidden = WHOLE.replace(
        "dashboard_group = \"Library\"\n",
        "dashboard_group = \"Library\"\ndashboard_hidden = true\n",
    );
    let manifest = parse(&hidden);
    let entry = manifest
        .as_ref()
        .and_then(|one| one.services.first().map(|service| one.entry(service)));
    assert_eq!(entry.map(|entry| entry.hidden), Some(true));
}

#[test]
fn a_wiring_naming_its_dashboard_line_and_icon_is_read_for_its_entry() {
    let named = WHOLE.replace(
        "dashboard_group = \"Library\"\n",
        "dashboard_group = \"Library\"\ndashboard_description = \"Comics\"\ndashboard_icon = \"komga.png\"\n",
    );
    let manifest = parse(&named);
    let entry = manifest
        .as_ref()
        .and_then(|one| one.services.first().map(|service| one.entry(service)));
    assert_eq!(entry.map(|entry| entry.description), Some("Comics"));
    assert_eq!(entry.map(|entry| entry.icon), Some(Some("komga.png")));
}
