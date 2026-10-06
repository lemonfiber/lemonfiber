use super::{Purpose, Scope, Wanted};

#[test]
fn a_scope_is_asked_for_in_one_of_three_words() {
    assert_eq!(Wanted::read("read"), Some(Wanted::Read));
    assert_eq!(Wanted::read(" act "), Some(Wanted::Act));
    assert_eq!(
        Wanted::read("member:alice"),
        Some(Wanted::Member("alice".to_owned()))
    );
    for refused in [
        "", "Read", "admin", "member:", "member: ", "operator", "read,act",
    ] {
        assert_eq!(Wanted::read(refused), None, "{refused}");
    }
}

#[test]
fn a_scope_is_written_the_way_it_is_asked_for() {
    assert_eq!(Scope::Read.written(), "read");
    assert_eq!(Scope::Act.written(), "act");
    let member = Scope::Member {
        id: "a1b2".to_owned(),
        name: "alice".to_owned(),
    };
    assert_eq!(member.written(), "member:alice");
    assert_eq!(member.member(), Some("a1b2"));
    assert_eq!(Scope::Act.member(), None);
}

#[test]
fn a_purpose_is_one_of_three_and_read_back_as_written() {
    for purpose in Purpose::EVERY {
        assert_eq!(Purpose::read(purpose.written()), Some(purpose));
        assert_eq!(
            serde_json::to_value(purpose).ok(),
            Some(serde_json::json!(purpose.written()))
        );
    }
    assert_eq!(Purpose::read("homeassistant"), None);
}
