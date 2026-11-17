//! The setting a run-and-exit service's schedule reads is a setting of its plugin's, and
//! takes only a time.

use crate::error::codes::setting::UNWRITABLE_VALUE;
use crate::scheduled::fixtures::{installed, SETTING};
use crate::settings::fences::timed;
use crate::settings::readers::{readers, SCHEDULE};

#[test]
fn the_setting_a_schedule_reads_is_listed_with_its_service_and_default() {
    let installed = installed(&[("configarr", Some("@daily"))]);
    let read = crate::test_support::stack()
        .manifest()
        .ok()
        .map(|manifest| readers(&manifest, &installed, &|_| None))
        .unwrap_or_default();
    let listed = read
        .settings
        .get(SETTING)
        .unwrap_or_else(|| unreachable!("listed"));
    let [one] = listed.as_slice() else {
        unreachable!("one reader: {listed:?}");
    };
    assert_eq!(one.service, "configarr");
    assert_eq!(one.variable, SCHEDULE);
    assert_eq!(one.default.as_deref(), Some("@daily"));
    assert!(one.met);
}

#[test]
fn a_value_naming_no_time_is_refused_naming_the_service() {
    let scheduled = crate::scheduled::all(&installed(&[("configarr", Some("@daily"))]));
    assert!(timed(&scheduled, SETTING, "30 4 * * 1-5").is_ok());
    assert!(timed(&scheduled, SETTING, "@weekly").is_ok());
    assert!(timed(&scheduled, SETTING, "").is_ok());
    assert!(timed(&scheduled, "configarr:other", "whenever").is_ok());
    let refused = timed(&scheduled, SETTING, "whenever")
        .err()
        .unwrap_or_else(|| unreachable!("refused"));
    assert_eq!(refused.code, UNWRITABLE_VALUE);
    assert!(refused.meaning.contains("configarr runs"));
    assert!(refused.meaning.contains("whenever"));
}
