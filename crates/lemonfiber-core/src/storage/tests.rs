#[test]
fn losing_hardlinks_is_stated_as_what_it_costs_rather_than_what_it_is() {
    // "Hardlinks unsupported" means nothing to most operators. What it means is
    // three concrete things, and all three have to be there — an operator
    // weighing whether to accept a location is weighing exactly these.
    let said = super::COPY_CONSEQUENCE.to_lowercase();
    assert!(said.contains("minutes"), "import time: {said}");
    assert!(said.contains("twice the disk"), "disk usage: {said}");
    assert!(said.contains("seed"), "seeding: {said}");
    // And it never leads with the property itself.
    assert!(!said.contains("hardlink"), "a filesystem property: {said}");
}
