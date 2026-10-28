use super::Https;

#[test]
fn only_an_https_address_naming_a_host_and_nothing_else_is_one() {
    let base = Https::parse("https://media:8920");
    assert_eq!(
        base.as_ref().map(ToString::to_string).as_deref(),
        Some("https://media:8920/")
    );
    assert!(base.is_some_and(|base| base.as_url().port() == Some(8920)));
    for refused in [
        "http://media:8920",
        "not an address",
        "https://user@media:8920",
        "https://user:secret@media:8920",
        "https://media:8920/?token=1",
        "https://media:8920/#part",
        "file:///etc/passwd",
    ] {
        assert_eq!(Https::parse(refused), None, "{refused}");
    }
}

#[test]
fn a_path_is_joined_beneath_the_address_and_nothing_can_leave_it() {
    let Some(base) = Https::parse("https://media:8920/jellyfin/") else {
        unreachable!("the base parses");
    };
    assert_eq!(
        base.joined("/Users/a7f3").map(|joined| joined.to_string()),
        Some("https://media:8920/jellyfin/Users/a7f3".to_owned())
    );
    for refused in [
        "Users",
        "//elsewhere/Users",
        "/Users?api_key=1",
        "/Users#part",
        "/../Users",
        "/Users/./a",
        "/Users\\a",
    ] {
        assert_eq!(base.joined(refused), None, "{refused}");
    }
}
