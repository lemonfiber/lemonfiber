use std::path::PathBuf;

use super::{asked, Asked, Settings};

/// Where the upstream these tests name answers by default.
const UPSTREAM: &str = "http://upstream:8080";

fn words(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_owned).collect()
}

#[test]
fn no_arguments_serve_the_stack_s_defaults() {
    assert_eq!(
        asked(Vec::new(), UPSTREAM),
        Ok(Asked::Serve(Settings::reaching(UPSTREAM)))
    );
}

#[test]
fn each_flag_sets_its_one_setting() {
    assert_eq!(
        asked(
            words(
                "--listen 127.0.0.1:9 --upstream http://127.0.0.1:8080 --configuration /tmp/here"
            ),
            UPSTREAM
        ),
        Ok(Asked::Serve(Settings {
            listen: "127.0.0.1:9".to_owned(),
            upstream: "http://127.0.0.1:8080".to_owned(),
            configuration: PathBuf::from("/tmp/here"),
        }))
    );
}

#[test]
fn health_asks_with_the_same_settings() {
    assert_eq!(
        asked(words("health --listen 127.0.0.1:9"), UPSTREAM),
        Ok(Asked::Health(Settings {
            listen: "127.0.0.1:9".to_owned(),
            ..Settings::reaching(UPSTREAM)
        }))
    );
}

#[test]
fn an_unknown_flag_or_a_missing_value_is_refused_by_name() {
    assert!(asked(words("--port 9"), UPSTREAM).is_err_and(|why| why.starts_with("--port")));
    assert!(asked(words("--listen"), UPSTREAM).is_err_and(|why| why.contains("needs a value")));
}
