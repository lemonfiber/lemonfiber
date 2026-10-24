use lemonfiber_ports::serving::StandIn;

use super::answer;

fn a_stand_in() -> StandIn {
    StandIn {
        port: 8080,
        asked_by: "mode".to_owned(),
        answers: vec![("version".to_owned(), r#"{"version":"4.5.1"}"#.to_owned())],
    }
}

#[test]
fn what_was_given_is_answered_by_the_value_of_its_parameter_and_nothing_else_is() {
    let stand_in = a_stand_in();

    assert_eq!(
        answer(&stand_in, "/api?apikey=k&mode=version&output=json"),
        ("200 OK", r#"{"version":"4.5.1"}"#)
    );
    assert_eq!(answer(&stand_in, "/api?mode=shutdown").0, "404 Not Found");
    assert_eq!(answer(&stand_in, "/api").0, "404 Not Found");
    assert_eq!(answer(&stand_in, "/api?modes=version").0, "404 Not Found");
}
