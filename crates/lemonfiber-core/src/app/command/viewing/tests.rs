use super::PlayFrom;
use crate::model::WITHHELD;

/// A device is shown as it is named, and a pass is never shown at all.
#[test]
fn a_device_is_debugged_by_name_and_a_pass_is_withheld() {
    assert_eq!(
        format!("{:?}", PlayFrom::Device("a-phone-0123".to_owned())),
        "Device(\"a-phone-0123\")"
    );
    let debugged = format!("{:?}", PlayFrom::Pass("a-sealed-pass".to_owned()));
    assert!(!debugged.contains("a-sealed-pass"), "{debugged}");
    assert!(debugged.contains(WITHHELD), "{debugged}");
}
