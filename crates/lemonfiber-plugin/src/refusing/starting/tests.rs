use super::super::tests::{names, said, INSTALLABLE};
use super::{LONGEST_ARG, MOST_ARGS};

const SERVICE: &str = "[[service]]\nid          = \"komga\"";

fn with_args(args: &str) -> Vec<String> {
    said(&INSTALLABLE.replace(SERVICE, &format!("{SERVICE}\nargs        = {args}")))
}

fn listed(args: &[String]) -> String {
    format!(
        "[{}]",
        args.iter()
            .map(|arg| format!("{arg:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

#[test]
fn constant_arguments_within_the_limits_are_accepted() {
    let most = vec!["serve".to_owned(); MOST_ARGS];
    let longest = vec!["a".repeat(LONGEST_ARG)];

    for args in [r#"["serve"]"#.to_owned(), listed(&most), listed(&longest)] {
        let said = with_args(&args);
        assert!(!names(&said, &["service komga.args"]), "{args}: {said:?}");
    }
}

#[test]
fn no_arguments_or_too_many_are_refused_naming_the_service() {
    let empty = with_args("[]");
    let many = with_args(&listed(&vec!["serve".to_owned(); MOST_ARGS + 1]));

    assert!(
        names(&empty, &["service komga.args", "is empty"]),
        "{empty:?}"
    );
    assert!(
        names(&many, &["service komga.args", "17 entries", "16"]),
        "{many:?}"
    );
}

#[test]
fn an_empty_long_substituting_or_controlling_argument_is_refused_naming_the_entry() {
    let long = "a".repeat(LONGEST_ARG + 1);
    for (arg, words) in [
        (String::new(), "is empty"),
        (long, "257 characters"),
        ("${HOME}".to_owned(), "substitution"),
        ("$HOME".to_owned(), "substitution"),
        ("{{secret:password}}".to_owned(), "substitution"),
        ("serve\n--x".to_owned(), "control character"),
    ] {
        let said = with_args(&listed(&["serve".to_owned(), arg.clone()]));
        assert!(
            names(&said, &["service komga.args[1]", words]),
            "{arg:?}: {said:?}"
        );
        assert!(
            !names(&said, &["service komga.args[0]"]),
            "{arg:?}: {said:?}"
        );
    }
}

#[test]
fn an_argument_that_is_not_a_string_is_refused() {
    let said = with_args(r#"["serve", 7]"#);

    assert!(names(&said, &["args"]), "{said:?}");
}

#[test]
fn an_argument_carrying_what_a_diff_cannot_show_is_refused_naming_it() {
    let said = with_args("[\"ser\\u202eve\"]");

    assert!(
        names(&said, &["service komga.args[0]", "U+202E"]),
        "{said:?}"
    );
}
