use serde_json::json;

use super::abridged;

#[test]
fn a_record_keeps_two_of_each_list_and_the_requirements_its_entries_cite() {
    let entry = |cites: &str| json!({ "summary": cites, "requirements": [cites] });
    let mut envelope = json!({ "data": { "changelog": {
        "releases": [{ "version": "3" }, { "version": "2" }, { "version": "1" }],
        "running": { "groups": [
            { "title": "a", "entries": [entry("A-R1"), entry("A-R2"), entry("A-R3")] },
            { "title": "b", "entries": [entry("B-R1")] },
            { "title": "c", "entries": [entry("C-R1")] },
        ] },
        "requirements": { "A-R1": {}, "A-R2": {}, "A-R3": {}, "B-R1": {}, "C-R1": {} },
    } } });

    abridged(&mut envelope);

    assert_eq!(
        envelope,
        json!({ "data": { "changelog": {
            "releases": [{ "version": "3" }, { "version": "2" }],
            "running": { "groups": [
                { "title": "a", "entries": [entry("A-R1"), entry("A-R2")] },
                { "title": "b", "entries": [entry("B-R1")] },
            ] },
            "requirements": { "A-R1": {}, "A-R2": {}, "B-R1": {} },
        } } })
    );
}

#[test]
fn a_record_without_its_lists_is_left_as_it_was() {
    let unread = json!({ "data": { "changelog": { "state": "stale", "running": null } } });
    let mut envelope = unread.clone();

    abridged(&mut envelope);

    assert_eq!(envelope, unread);
}
