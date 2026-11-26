use std::sync::Arc;

use lemonfiber_ports::http::Sessions;
use reqwest::cookie::CookieStore;
use reqwest::header::HeaderValue;
use reqwest::Url;

use super::{Fresh, PerOrigin};

#[test]
fn each_session_opens_a_client_of_its_own() {
    assert!(!Arc::ptr_eq(&Fresh.opened(), &Fresh.opened()));
}

#[test]
fn an_origin_that_set_two_cookies_is_sent_both_and_another_port_is_sent_neither() {
    let jar = PerOrigin::default();
    let signed_in = [
        HeaderValue::from_static("SID=signed-in; Path=/"),
        HeaderValue::from_static("lang=en"),
    ];
    if let Ok(login) = Url::parse("http://127.0.0.1:8080/login") {
        jar.set_cookies(&mut signed_in.iter(), &login);
    }
    let returned = |at: &str| Url::parse(at).ok().and_then(|at| jar.cookies(&at));
    assert_eq!(
        returned("http://127.0.0.1:8080/queue"),
        Some(HeaderValue::from_static("SID=signed-in; lang=en"))
    );
    assert_eq!(returned("http://127.0.0.1:9090/queue"), None);
}
