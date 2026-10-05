use super::{wget, ECHO_WITHIN_SECONDS};

/// An echo is asked with a bound of its own, so a tunnel dropping packets is
/// answered within it rather than after the whole TCP timeout.
#[test]
fn an_echo_is_asked_with_a_bound_of_its_own() {
    let asked = wget("https://echo.example".to_owned());
    let bound = asked
        .iter()
        .position(|word| word == "-T")
        .and_then(|at| asked.get(at + 1));
    assert_eq!(bound, Some(&ECHO_WITHIN_SECONDS.to_string()), "{asked:?}");
    assert_eq!(
        asked.last().map(String::as_str),
        Some("https://echo.example")
    );
}
