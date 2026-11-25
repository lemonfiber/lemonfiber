use super::{reads_as_off, reads_as_on};

#[test]
fn a_setting_may_be_spelled_the_ways_people_spell_it() {
    // Including quoted, which a hand-edited .env commonly is.
    for on in ["on", "ON", "true", "yes", "1", " on ", "\"on\"", "'yes'"] {
        assert!(reads_as_on(on), "{on:?} should read as on");
    }
    for off in ["off", "false", "no", "0", "", "maybe", "\"off\"", "\"\""] {
        assert!(!reads_as_on(off), "{off:?} should not read as on");
        if off != "maybe" {
            assert!(reads_as_off(off), "{off:?} should read as off");
        }
    }
}
