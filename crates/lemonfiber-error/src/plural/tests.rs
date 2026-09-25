use super::s;

#[test]
fn one_is_singular_and_everything_else_is_not() {
    assert_eq!(s(1), "");
    assert_eq!(s(0), "s", "no others, not no other");
    assert_eq!(s(2), "s");
    assert_eq!(s(usize::MAX), "s");
}
