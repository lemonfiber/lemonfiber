use super::environment_name;

#[test]
fn an_id_is_upper_cased_and_its_separators_written_alike() {
    assert_eq!(
        environment_name("calibre-web-automated"),
        "CALIBRE_WEB_AUTOMATED"
    );
    assert_eq!(environment_name("foo_bar"), environment_name("foo-bar"));
    assert_eq!(environment_name("Sonarr"), environment_name("sonarr"));
}
