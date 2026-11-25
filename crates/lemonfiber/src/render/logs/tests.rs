use lemonfiber_core::app::exporting::Exported;

use super::exported;

#[test]
fn an_export_says_where_it_went_or_would_have_gone() {
    let written = Exported {
        path: "lemonfiber-logs-1-1.txt".to_owned(),
        rehearsed: false,
    };
    assert_eq!(
        exported(&written).text(),
        "written to lemonfiber-logs-1-1.txt"
    );
    let rehearsed = Exported {
        rehearsed: true,
        ..written
    };
    assert_eq!(
        exported(&rehearsed).text(),
        "would be written to lemonfiber-logs-1-1.txt"
    );
}
