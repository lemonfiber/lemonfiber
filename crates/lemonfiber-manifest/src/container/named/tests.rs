use super::wrong;

#[test]
fn a_label_beginning_with_a_letter_is_a_container_s_name() {
    for good in ["bazarr", "a", "subtitles-1", "x9"] {
        assert_eq!(wrong(good), None, "{good}");
    }
}

#[test]
fn a_name_that_is_no_label_or_reads_as_a_number_is_refused_saying_why() {
    for (bad, why) in [
        ("", "not one DNS label"),
        ("Bazarr", "not one DNS label"),
        ("bazarr-", "not one DNS label"),
        ("baz.arr", "not one DNS label"),
        (&"a".repeat(64), "not one DNS label"),
        ("77e670703184", "does not begin with a letter"),
        ("1e5", "does not begin with a letter"),
        ("-bazarr", "not one DNS label"),
        ("inf", "reads as a number"),
        ("infinity", "reads as a number"),
        ("nan", "reads as a number"),
    ] {
        assert!(
            wrong(bad).is_some_and(|said| said.contains(why)),
            "{bad}: {:?}",
            wrong(bad)
        );
    }
}
