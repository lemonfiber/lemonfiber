use super::query_encoded;

#[test]
fn a_query_value_keeps_only_unreserved_bytes_and_writes_a_space_as_a_percent() {
    assert_eq!(
        query_encoded("a-b.c_d~e f+g&h=i/é"),
        "a-b.c_d~e%20f%2Bg%26h%3Di%2F%C3%A9"
    );
}
