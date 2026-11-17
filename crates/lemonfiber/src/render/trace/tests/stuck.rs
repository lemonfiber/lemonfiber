//! The stuck list: each item stuck, and where to trace it.

use super::*;

#[test]
fn the_stuck_list_names_each_item_and_links_its_trace() {
    let report = StuckReport {
        items: vec![StuckEntry {
            title: "The Expanse".to_owned(),
            service: "Sonarr".to_owned(),
            stage: Stage::Downloading,
        }],
        incomplete: true,
    };
    let text = stuck(&report).text();
    assert!(text.contains("1 item(s) stuck"));
    assert!(text.contains("stuck at downloading"));
    assert!(text.contains("trace 'The Expanse'"));
    assert!(text.contains("may be incomplete"));

    // Nothing stuck is said plainly.
    let clear = StuckReport {
        items: Vec::new(),
        incomplete: false,
    };
    assert!(stuck(&clear).text().contains("Nothing is stuck"));
}
